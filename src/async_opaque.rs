//! 异步类型无关中间件链 —— **async × 任意类型** 同链（CONFIRM-SWEEP 边 #1）
//!
//! `opaque::OpaqueChain` 是同步；`async_mw` 是 i32。本模块让**任意共享 repr(C) 类型**
//! 在**异步链**上执行：
//! - 同步节点（运行期编译插件 `OpaqueNode::Thin` / 宿主薄变换 / 治理 `Stateful`）→ 同步调用
//! - 异步有状态节点（`OpaqueAsyncMw`）→ 真实 `await`
//!
//! **边界（D6，显式记录）**：`extern "C"` 无法安全导出 async fn，因此"运行期编译 +
//! 真实 await"的插件是显式边界。三者（async × 任意类型 × 运行期编译）同时成立的部分 =
//! **运行期编译同步插件进异步链 + 宿主侧异步节点承担 await**。异步逻辑若需运行期
//! 热更，落宿主侧 `OpaqueAsyncMw` 实现（Stateful，可热换实例）。

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::opaque::{OpaqueNode, OPAQUE_BREAK, OPAQUE_CONTINUE};

/// 异步类型无关中间件（有状态；`*mut c_void` 是裸指针，可安全跨 await 持有）
pub trait OpaqueAsyncMw: Send + Sync {
    /// 进入（可真实 await）；返回 0 继续 / 1 短路 / 2 拒绝
    fn call<'a>(
        &'a self,
        req: *mut std::ffi::c_void,
    ) -> Pin<Box<dyn Future<Output = i32> + Send + 'a>>;
    /// 退出（洋葱逆序，仅成功路径）
    fn exit(&self, _req: *mut std::ffi::c_void) {}
}

/// 异步链节点：同步（运行期编译/治理）或异步（宿主有状态）
#[derive(Clone)]
pub enum OpaqueAsyncNode {
    /// 同步节点：运行期编译插件（Thin）/ 治理（Stateful）——在异步链中同步调用
    Sync(OpaqueNode),
    /// 异步有状态节点：真实 await
    Async(Arc<dyn OpaqueAsyncMw>),
}

/// 异步类型无关中间件链（RCU 快照：add/remove/set 热替换）
#[derive(Clone)]
pub struct OpaqueAsyncChain {
    nodes: Arc<Vec<OpaqueAsyncNode>>,
}

impl OpaqueAsyncChain {
    pub fn new(nodes: Vec<OpaqueAsyncNode>) -> Self {
        OpaqueAsyncChain { nodes: Arc::new(nodes) }
    }

    pub fn empty() -> Self {
        OpaqueAsyncChain { nodes: Arc::new(Vec::new()) }
    }

    pub fn add(&mut self, node: OpaqueAsyncNode) {
        let mut v = (*self.nodes).clone();
        v.push(node);
        self.nodes = Arc::new(v);
    }

    pub fn set(&mut self, idx: usize, node: OpaqueAsyncNode) -> bool {
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

    /// 异步执行：sync 节点同步调（含运行期编译插件），async 节点真实 await；
    /// 核心后 exit 逆序洋葱（sync 的 exit/stateful exit/async exit）。
    pub async fn exec<R, O>(&self, core: fn(&mut R) -> O, req: &mut R) -> Result<O, i32> {
        let nodes = self.nodes.as_ref();
        let ptr = req as *mut R as *mut std::ffi::c_void;
        for n in nodes.iter() {
            let code = match n {
                OpaqueAsyncNode::Sync(sn) => match sn {
                    OpaqueNode::Thin { enter, .. } => unsafe { (enter)(ptr, std::ptr::null_mut()) },
                    OpaqueNode::Stateful(mw) => mw.enter(ptr),
                },
                OpaqueAsyncNode::Async(mw) => mw.call(ptr).await,
            };
            match code {
                OPAQUE_CONTINUE => {}
                OPAQUE_BREAK => return Err(OPAQUE_BREAK),
                _ => return Err(code),
            }
        }
        let out = core(req);
        for n in nodes.iter().rev() {
            match n {
                OpaqueAsyncNode::Sync(OpaqueNode::Thin { exit: Some(f), .. }) => unsafe { f(ptr) },
                OpaqueAsyncNode::Sync(OpaqueNode::Stateful(mw)) => mw.exit(ptr),
                OpaqueAsyncNode::Async(mw) => mw.exit(ptr),
                _ => {}
            }
        }
        Ok(out)
    }
}
