//! 实验：D8 迁移工具链 — 6 个已有 handler 批量经 `adopt` 搬上中间件层（加性可回滚）
//!
//! `migrate::adopt` 是通用采纳点：原 handler 不变，只在其外层加链（加性）；
//! 不 adopt 即回滚（原 handler 原样可用）。这落地 D8"包装核心 → 渐进采纳"。
//!
//! 跑：`cd labs/scales/micro && cargo run --release --bin exp_d8_migrate`

use std::sync::Arc;

use proc_mw::compile::build_plugin_with_deps;
use proc_mw::migrate::adopt;
use proc_mw::opaque::{OpaqueChain, OpaqueNode};
use proc_mw::opaque_gov::OpaqueMetrics;
use proc_mw::runtime::PluginOpaque;

use micro_service::MegaReq;

const HOST_DEPS: &str = concat!(
    "micro_service = { path = \"",
    env!("CARGO_MANIFEST_DIR"),
    "\" }"
);

// 已有 6 个 handler（无中间件，D8 迁移对象）
fn handle_login(v: i64) -> i64 { v + 1000 }
fn handle_get_user(v: i64) -> i64 { v * 2 }
fn handle_create_order(v: i64) -> i64 { v + 500 }
fn handle_list_orders(v: i64) -> i64 { v / 2 }
fn handle_update_profile(v: i64) -> i64 { v + 10 }
fn handle_delete_user(v: i64) -> i64 { -v }

fn main() {
    // 编译审计插件（直接依赖宿主）
    let src = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/exp_mega_chain/mw_audit.rs"));
    let so = build_plugin_with_deps("d8_audit", src, HOST_DEPS, &std::env::temp_dir()).unwrap();
    let audit = PluginOpaque::load(so.to_str().unwrap()).unwrap();
    let metrics = Arc::new(OpaqueMetrics::new());
    let chain = OpaqueChain::new(vec![OpaqueNode::Stateful(metrics.clone()), audit.to_node()]);
    println!("[1] 链就绪：metrics + audit 插件（直接依赖宿主）");

    // D8 采纳：6 个 handler 批量经 adopt 搬上中间件层（输入 1 → 审计 +5）
    let handlers: [fn(i64) -> i64; 6] = [
        handle_login, handle_get_user, handle_create_order,
        handle_list_orders, handle_update_profile, handle_delete_user,
    ];
    let mk = |v: i64| MegaReq { value: v, trace_id: 0, deadline_ms: u64::MAX, audited: false };
    let migrated: Vec<i64> = handlers
        .iter()
        .map(|h| adopt(&chain, 1, mk, |r| h(r.value)).unwrap())
        .collect();
    println!("[2] 6 handler 经 adopt 采纳（审计 +5，输入 1→6）：{migrated:?}");
    assert_eq!(migrated[0], 1006, "login 收到审计后 6");
    assert_eq!(metrics.calls(), 6, "6 个采纳点各执行一次链");

    // 可回滚：原 handler 不经链直接调用（未变）
    assert_eq!(handle_login(1), 1001, "回滚 = 不 adopt，原 handler 原样");
    assert_eq!(handle_get_user(1), 2);

    println!("---");
    println!("D8 迁移实验通过：adopt 批量采纳 6 handler（加性、可回滚）✓");
}
