//! D3 动态性 · 数据/快照层增删（RCU）+ 生产形状洋葱执行
//!
//! 链是不可变快照（Arc 持有），add/remove = 复制快照 + Arc 原子替换。
//! 读路径只 deref Arc + 迭代——无锁、无分配。
//! 核心由调用方注入，链可复用于任意核心。

use std::sync::Arc;

use crate::dispatch::{chain_exec, Ctx, MwError, Node};

/// 中间件链 = 不可变快照
#[derive(Clone)]
pub struct Chain {
    nodes: Arc<Vec<Node>>,
}

impl Chain {
    pub fn new(nodes: Vec<Node>) -> Self {
        Chain {
            nodes: Arc::new(nodes),
        }
    }

    /// 读路径：无锁、无分配；洋葱模型执行（enter 正序 → 核心 → exit 逆序）
    pub fn exec(
        &self,
        core: impl Fn(&mut Ctx) -> Result<i32, MwError>,
        input: i32,
    ) -> Result<i32, MwError> {
        chain_exec(&self.nodes, core, input)
    }

    /// 恢复原语（L5）：错误时用 fallback 替代，不向调用方暴露错误
    pub fn exec_or(
        &self,
        core: impl Fn(&mut Ctx) -> Result<i32, MwError>,
        input: i32,
        fallback: fn(i32) -> i32,
    ) -> i32 {
        match self.exec(core, input) {
            Ok(v) => v,
            Err(_) => fallback(input),
        }
    }

    /// panic 恢复：核心（Rust 代码，可展开）panic 时 catch 住 → MwError，
    /// 链保持可用（类似 web 框架 catch 处理器 panic）。
    /// 注意：仅对 Rust 核心有效；extern C 插件 panic 是 abort（L3），catch 无效。
    pub fn exec_catch(
        &self,
        core: impl Fn(&mut Ctx) -> Result<i32, MwError>,
        input: i32,
    ) -> Result<i32, MwError> {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.exec(core, input)
        }));
        match result {
            Ok(r) => r,
            Err(_) => Err(MwError::Rejected("core panicked")),
        }
    }

    /// 写路径：复制快照 + 原子替换（RCU），Θ(len)，稀有操作
    pub fn add(&mut self, node: Node) {
        let mut v = (*self.nodes).clone();
        v.push(node);
        self.nodes = Arc::new(v);
    }
    pub fn remove(&mut self, idx: usize) {
        let mut v = (*self.nodes).clone();
        v.remove(idx);
        self.nodes = Arc::new(v);
    }

    /// 快照内替换节点（D6 热替换：换插件逻辑不停机）
    pub fn set(&mut self, idx: usize, node: Node) -> bool {
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
}
