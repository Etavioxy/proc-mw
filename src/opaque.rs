//! D6 类型无关中间件链 —— **核心目的的落地**：运行期编译任意 Rust 代码，
//! 操作**任意共享类型**，粘合进中间层。
//!
//! `dispatch::Chain`（`Ctx { input: i32 }`）是控制面治理链（metrics/限流/追踪）。
//! 本模块是**数据面**：请求类型由宿主与插件**各自定义同一 `#[repr(C)]` 布局**的
//! 共享类型决定，ABI 用 `*mut c_void`（类型擦除指针）。插件内部对任意类型调用方法
//! （`String::push` / `Vec::sort` / struct 字段），运行期编译 → dlopen → RCU 快照热替换。
//!
//! ```text
//! 宿主                        插件（运行期编译，任意 Rust）
//! repr(C) struct Msg {..}  ⇄  repr(C) struct Msg {..}   ← 共享类型定义（布局一致）
//!   &mut Msg ──c_void──▶  mw_enter(req: *mut c_void, resp: *mut c_void)
//! ```
//!
//! 语义对齐 `dispatch::chain_exec`：enter 正序（可短路/报错，报错不跑 exit）→
//! core → exit 逆序（洋葱）。契约（D7）：`extern "C"` + ABI 版本符号 + enter 返回码
//! （0 继续 / 1 短路 / 2 拒绝），插件永不 panic，错误经返回码传播。
//! 热替换永不 unload（防 TLS destructor），`keepalive` 保活句柄随节点走。

use std::any::Any;
use std::sync::Arc;

/// 类型无关中间件节点（D2 槽位：Extern——thin fn 指针 + 保活句柄，无 vtable）
#[derive(Clone)]
pub struct OpaqueNode {
    /// 进入变换：`req` 指向共享类型实例，`resp` 预留输出缓冲（当前契约为 null）
    pub enter: unsafe extern "C" fn(*mut std::ffi::c_void, *mut std::ffi::c_void) -> i32,
    /// 退出钩子（洋葱逆序，可改写请求）
    pub exit: Option<unsafe extern "C" fn(*mut std::ffi::c_void)>,
    /// 保活句柄（Arc<Library> 类型擦除）——热替换永不 unload
    pub keepalive: Arc<dyn Any + Send + Sync>,
}

/// enter 返回码契约（D7）：0 继续 / 1 短路 / 2 拒绝
pub const OPAQUE_CONTINUE: i32 = 0;
pub const OPAQUE_BREAK: i32 = 1;
pub const OPAQUE_REJECT: i32 = 2;

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
    /// 正常路径 = enter 正序 → core → exit 逆序洋葱。
    pub fn exec<R, O>(&self, core: impl Fn(&mut R) -> O, req: &mut R) -> Result<O, i32> {
        let nodes = self.nodes.as_ref();
        let ptr = req as *mut R as *mut std::ffi::c_void;
        for n in nodes.iter() {
            let code = unsafe { (n.enter)(ptr, std::ptr::null_mut()) };
            match code {
                OPAQUE_CONTINUE => {}
                OPAQUE_BREAK => return Err(OPAQUE_BREAK), // 短路：终止
                _ => return Err(code),                    // 拒绝/错误：经返回码传播
            }
        }
        let out = core(req);
        for n in nodes.iter().rev() {
            if let Some(exit) = n.exit {
                unsafe { exit(ptr) };
            }
        }
        Ok(out)
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

    fn node(f: unsafe extern "C" fn(*mut std::ffi::c_void, *mut std::ffi::c_void) -> i32) -> OpaqueNode {
        OpaqueNode {
            enter: f,
            exit: None,
            keepalive: Arc::new(()),
        }
    }

    #[test]
    fn opaque_chain_transform_and_hotswap() {
        // 任意类型共享 struct 经链变换（enter 正序）
        let mut chain = OpaqueChain::new(vec![node(add1), node(add10)]);
        let mut m = Msg { val: 0, hops: 0 };
        let r = chain.exec(|m| m.val, &mut m).unwrap();
        assert_eq!(r, 11);
        assert_eq!(m.hops, 2, "两个节点各 hop 一次");
        // RCU 热替换：槽位 0 add1 → add10（行为变化，快照替换）
        assert!(chain.set(0, node(add10)));
        let mut m2 = Msg { val: 0, hops: 0 };
        assert_eq!(chain.exec(|m| m.val, &mut m2).unwrap(), 20);
        // 拒绝码传播：拒绝后不再执行后续节点（D7 错误经返回码）
        let chain2 = OpaqueChain::new(vec![node(reject), node(add1)]);
        let mut m3 = Msg { val: 0, hops: 0 };
        assert_eq!(chain2.exec(|m| m.val, &mut m3), Err(OPAQUE_REJECT));
        assert_eq!(m3.hops, 0, "拒绝后后续节点不执行");
    }
}
