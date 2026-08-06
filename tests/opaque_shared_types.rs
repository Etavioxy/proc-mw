//! 直接共享类型路径（usergoals）：插件经 **crate 依赖**引用宿主类型，非双写声明
//!
//! 证明：**非 repr(C)** 类型（`String` 堆字段 + enum 字段）经 c_void 在宿主与运行期
//! 编译插件间直接共享——同一类型定义（同一 crate/同一编译器）→ 布局必然一致 →
//! c_void 转换安全，**不需要手工镜像布局**。这是 bevy ECS 类型兼容的路径
//! （组件/资源/事件通常非 repr(C)、布局复杂）。

use proc_mw::compile::build_plugin_with_deps;
use proc_mw::opaque::OpaqueChain;
use proc_mw::runtime::PluginOpaque;
use shared_types::{EventKind, GameEvent};

/// 插件 crate 的依赖：直接路径依赖 shared_types crate
const SHARED_DEPS: &str = concat!(
    "shared_types = { path = \"",
    env!("CARGO_MANIFEST_DIR"),
    "/labs/shared_types\" }"
);

const PLUGIN_SRC: &str = r#"
use shared_types::{GameEvent, EventKind};

#[no_mangle] pub extern "C" fn proc_mw_abi_version() -> i32 { 1 }
#[no_mangle] pub unsafe extern "C" fn mw_enter(req: *mut std::ffi::c_void, _resp: *mut std::ffi::c_void) -> i32 {
    let e = unsafe { &mut *(req as *mut GameEvent) };
    e.text.push_str(" [audited]");                       // 非 repr(C) 类型的 String 字段
    if e.kind == EventKind::Buy { e.ts_ms = 0; }          // enum 字段匹配
    0
}
"#;

#[test]
fn plugin_shares_non_repr_c_type_via_crate_dep() {
    let so = build_plugin_with_deps("shared_types_demo", PLUGIN_SRC, SHARED_DEPS, &std::env::temp_dir())
        .expect("运行期编译直接共享类型插件（依赖 shared_types crate）");
    let p = PluginOpaque::load(so.to_str().unwrap()).unwrap();
    let chain = OpaqueChain::new(vec![p.to_node()]);
    // 直接使用共享类型（无镜像声明）
    let mut e = GameEvent { id: 7, kind: EventKind::Buy, text: "sword".into(), ts_ms: 1234 };
    let r = chain.exec(|e| e.id, &mut e).unwrap();
    assert_eq!(r, 7);
    assert!(e.text.contains("[audited]"), "插件直接操作共享 String 字段");
    assert_eq!(e.ts_ms, 0, "Buy 事件 ts_ms 归零（enum 匹配生效）");
    // 非 Buy 事件不触发
    let mut e2 = GameEvent { id: 8, kind: EventKind::Move, text: "run".into(), ts_ms: 99 };
    chain.exec(|e| e.id, &mut e2).unwrap();
    assert_eq!(e2.ts_ms, 99, "Move 不触发 ts_ms 归零");
    assert!(e2.text.contains("[audited]"));
    // 热替换：同一类型换逻辑
    let so2 = build_plugin_with_deps("shared_types_demo2", PLUGIN_SRC, SHARED_DEPS, &std::env::temp_dir()).unwrap();
    let p2 = PluginOpaque::load(so2.to_str().unwrap()).unwrap();
    let mut chain2 = OpaqueChain::new(vec![p.to_node()]);
    assert!(chain2.set(0, p2.to_node()));
    let mut e3 = GameEvent { id: 9, kind: EventKind::Login, text: "hi".into(), ts_ms: 5 };
    chain2.exec(|e| e.id, &mut e3).unwrap();
    assert_eq!(e3.id, 9);
    assert!(e3.text.contains("[audited]"));
}
