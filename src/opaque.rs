//! D6 类型无关中间件链 —— **核心目的的落地**：运行期编译任意 Rust 代码，
//! 操作**任意共享类型**，粘合进中间层。
//!
//! `dispatch::Chain`（`Ctx { input: i32 }`）是控制面治理链。本模块是**类型无关**的
//! 中间层：请求类型由宿主与插件**各自定义同一 `#[repr(C)]` 布局**的共享类型决定，
//! ABI 用 `*mut c_void`（类型擦除指针）。插件内部对任意类型调用方法
//! （`String::push` / `Vec::sort` / struct 字段），运行期编译 → dlopen → RCU 快照热替换。
//!
//! ```text
//! 宿主                        插件（运行期编译，任意 Rust）
//! repr(C) struct Msg {..}  ⇄  repr(C) struct Msg {..}   ← 共享类型定义（布局一致）
//!   &mut Msg ──c_void──▶  mw_enter(req: *mut c_void, resp: *mut c_void)
//! ```
//!
//! **节点槽位（D2 状态承载 → 独立可选）**：
//! - `Thin`：无状态（运行期编译插件落槽 / 宿主薄变换），extern "C" fn 对 + 保活句柄
//! - `Stateful`：有状态治理（metrics/限流/熔断），`Arc<dyn OpaqueMw>`——不再绑 i32 Ctx
//!
//! 语义对齐 `dispatch::chain_exec`：enter 正序（可短路/报错，报错不跑 exit）→
//! core → exit 逆序（洋葱）。契约（D7）：`extern "C"` + ABI 版本符号 + enter 返回码
//! （0 继续 / 1 短路 / 2 拒绝），插件永不 panic，错误经返回码传播。
//! 热替换永不 unload（防 TLS destructor），`keepalive` 保活句柄随节点走。

use std::any::Any;
use std::sync::Arc;

/// enter 返回码契约（D7）：0 继续 / 1 短路 / 2 拒绝
pub const OPAQUE_CONTINUE: i32 = 0;
pub const OPAQUE_BREAK: i32 = 1;
pub const OPAQUE_REJECT: i32 = 2;

/// 有状态类型无关中间件（治理：metrics/限流/熔断）。D2：状态承载 → dyn 槽位。
pub trait OpaqueMw: Send + Sync {
    /// 进入：`req` 指向共享类型实例。返回 0 继续 / 1 短路 / 2 拒绝。
    fn enter(&self, req: *mut std::ffi::c_void) -> i32;
    /// 退出（洋葱逆序，仅成功路径调用；错误短路不达）
    fn exit(&self, _req: *mut std::ffi::c_void) {}
}

/// 封闭内联槽位（D2：位标记/开关——对齐 Ctx 链的 `Builtin` enum，但类型无关）。
/// 类型无关操作只有：通过 / 短路 / 拒绝（无状态，实现为 Stateful 但零状态）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpaqueBuiltin {
    /// 通过（no-op）：占位/开关打开
    Continue,
    /// 短路：终止链（返回码 1）
    Break,
    /// 拒绝：错误（返回码 2）——开关关闭/熔断注入
    Reject,
}

impl OpaqueMw for OpaqueBuiltin {
    fn enter(&self, _req: *mut std::ffi::c_void) -> i32 {
        match self {
            OpaqueBuiltin::Continue => OPAQUE_CONTINUE,
            OpaqueBuiltin::Break => OPAQUE_BREAK,
            OpaqueBuiltin::Reject => OPAQUE_REJECT,
        }
    }
}

impl OpaqueBuiltin {
    /// 产出 Stateful 槽位节点（零状态，但走统一的 Stateful 分发）
    pub fn to_node(self) -> OpaqueNode {
        OpaqueNode::Stateful(Arc::new(self))
    }
}

/// 类型无关中间件节点（D2 槽位：Thin fn-ptr 或 Stateful dyn）
#[derive(Clone)]
pub enum OpaqueNode {
    /// 无状态：extern "C" 变换对 + 保活句柄（运行期编译插件 / 宿主薄变换）
    Thin {
        enter: unsafe extern "C" fn(*mut std::ffi::c_void, *mut std::ffi::c_void) -> i32,
        exit: Option<unsafe extern "C" fn(*mut std::ffi::c_void)>,
        keepalive: Arc<dyn Any + Send + Sync>,
    },
    /// 有状态：治理中间件（metrics/限流），不绑 i32 Ctx
    Stateful(Arc<dyn OpaqueMw>),
}

/// 类型无关中间件链（RCU 快照：读路径无锁、无分配；add/remove/set = 替换快照）
#[derive(Clone)]
pub struct OpaqueChain {
    nodes: Arc<Vec<OpaqueNode>>,
}

impl OpaqueChain {
    pub fn new(nodes: Vec<OpaqueNode>) -> Self {
        OpaqueChain { nodes: Arc::new(nodes) }
    }

    /// 空链（D4 空链透明对照）
    pub fn empty() -> Self {
        OpaqueChain { nodes: Arc::new(Vec::new()) }
    }

    /// RCU 快照增：追加节点
    pub fn add(&mut self, node: OpaqueNode) {
        let mut v = (*self.nodes).clone();
        v.push(node);
        self.nodes = Arc::new(v);
    }

    /// RCU 快照删
    pub fn remove(&mut self, idx: usize) {
        let mut v = (*self.nodes).clone();
        if idx < v.len() {
            v.remove(idx);
        }
        self.nodes = Arc::new(v);
    }

    /// 热替换：快照中指定位置换成新节点（不停机，读路径无锁）
    pub fn set(&mut self, idx: usize, node: OpaqueNode) -> bool {
        let mut v = (*self.nodes).clone();
        if idx < v.len() {
            v[idx] = node;
            self.nodes = Arc::new(v);
            true
        } else {
            false
        }
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// 数据面执行：`req` 经全链变换后再跑 `core`。
    /// `R` = 共享类型（`#[repr(C)]`，宿主与插件各自定义同一布局），`O` = 业务返回。
    /// 语义对齐 `dispatch::chain_exec`：enter 短路/报错即终止（不跑 core / exit）；
    /// 正常路径 = enter 正序 → core → exit 逆序洋葱（stateful 同样收到 exit）。
    pub fn exec<R, O>(&self, core: impl Fn(&mut R) -> O, req: &mut R) -> Result<O, i32> {
        let nodes = self.nodes.as_ref();
        let ptr = req as *mut R as *mut std::ffi::c_void;
        for n in nodes.iter() {
            let code = match n {
                OpaqueNode::Thin { enter, .. } => unsafe { (enter)(ptr, std::ptr::null_mut()) },
                OpaqueNode::Stateful(mw) => mw.enter(ptr),
            };
            match code {
                OPAQUE_CONTINUE => {}
                OPAQUE_BREAK => return Err(OPAQUE_BREAK), // 短路：终止
                _ => return Err(code),                    // 拒绝/错误：经返回码传播
            }
        }
        let out = core(req);
        for n in nodes.iter().rev() {
            match n {
                OpaqueNode::Thin { exit: Some(f), .. } => unsafe { f(ptr) },
                OpaqueNode::Stateful(mw) => mw.exit(ptr),
                _ => {}
            }
        }
        Ok(out)
    }

    /// 重试执行（语义原语，对齐 `chain::exec_retry`）：链失败（返回码 ≠0）时重试
    /// 至多 `n` 次，成功立即返回。请求每次重试重新过链（无状态变换可安全重放）。
    pub fn exec_retry<R, O>(
        &self,
        core: impl Fn(&mut R) -> O,
        req: &mut R,
        n: u32,
    ) -> Result<O, i32> {
        let mut last_err = 0i32;
        for _ in 0..n {
            match self.exec(&core, req) {
                Ok(v) => return Ok(v),
                Err(e) => last_err = e,
            }
        }
        Err(last_err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[repr(C)]
    struct Msg {
        val: i64,
        hops: u32,
    }

    unsafe extern "C" fn add1(req: *mut std::ffi::c_void, _: *mut std::ffi::c_void) -> i32 {
        let m = unsafe { &mut *(req as *mut Msg) };
        m.val += 1;
        m.hops += 1;
        OPAQUE_CONTINUE
    }
    unsafe extern "C" fn add10(req: *mut std::ffi::c_void, _: *mut std::ffi::c_void) -> i32 {
        let m = unsafe { &mut *(req as *mut Msg) };
        m.val += 10;
        m.hops += 1;
        OPAQUE_CONTINUE
    }
    unsafe extern "C" fn reject(_req: *mut std::ffi::c_void, _: *mut std::ffi::c_void) -> i32 {
        OPAQUE_REJECT
    }

    fn thin(
        f: unsafe extern "C" fn(*mut std::ffi::c_void, *mut std::ffi::c_void) -> i32,
    ) -> OpaqueNode {
        OpaqueNode::Thin {
            enter: f,
            exit: None,
            keepalive: Arc::new(()),
        }
    }

    #[test]
    fn opaque_chain_transform_and_hotswap() {
        // 任意类型共享 struct 经链变换（enter 正序）
        let mut chain = OpaqueChain::new(vec![thin(add1), thin(add10)]);
        let mut m = Msg { val: 0, hops: 0 };
        let r = chain.exec(|m| m.val, &mut m).unwrap();
        assert_eq!(r, 11);
        assert_eq!(m.hops, 2, "两个节点各 hop 一次");
        // RCU 热替换：槽位 0 add1 → add10（行为变化，快照替换）
        assert!(chain.set(0, thin(add10)));
        let mut m2 = Msg { val: 0, hops: 0 };
        assert_eq!(chain.exec(|m| m.val, &mut m2).unwrap(), 20);
        // 拒绝码传播：拒绝后不再执行后续节点（D7 错误经返回码）
        let chain2 = OpaqueChain::new(vec![thin(reject), thin(add1)]);
        let mut m3 = Msg { val: 0, hops: 0 };
        assert_eq!(chain2.exec(|m| m.val, &mut m3), Err(OPAQUE_REJECT));
        assert_eq!(m3.hops, 0, "拒绝后后续节点不执行");
    }
}
