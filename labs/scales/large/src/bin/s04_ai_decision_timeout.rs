//! 场景 S04 · AI 决策超时（large 档，bevy_ecs 事件流 + deadline 插件热更）
//!
//! AiDecision 经 OpaqueChain（OpaqueMetrics + deadline 插件 v1→v2，直接依赖宿主）进
//! bevy `Events<AiDecision>`；超时被拒（不达 bevy，AI 走默认动作）。
//!
//! 跑：`cd labs/scales/large && cargo run --release --bin s04_ai_decision_timeout`

use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use bevy_ecs::event::{EventReader, Events};
use bevy_ecs::prelude::*;
use bevy_ecs::schedule::Schedule;
use proc_mw::compile::build_plugin_with_deps;
use proc_mw::opaque::{OpaqueChain, OpaqueNode};
use proc_mw::opaque_gov::OpaqueMetrics;
use proc_mw::runtime::PluginOpaque;

use large_service::AiDecision;

const HOST_DEPS: &str = concat!(
    "large_service = { path = \"",
    env!("CARGO_MANIFEST_DIR"),
    "\" }"
);

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64
}

#[derive(Resource, Default)]
struct Decided(u32);

fn consumer(mut r: EventReader<AiDecision>, mut decided: ResMut<Decided>) {
    for ev in r.read() {
        if ev.decided {
            decided.0 += 1;
        }
    }
}

fn main() {
    let src_v1 = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/s04_ai_decision_timeout/mw_v1.rs"));
    let so1 = build_plugin_with_deps("large_ai_v1", src_v1, HOST_DEPS, &std::env::temp_dir()).unwrap();
    let v1 = PluginOpaque::load(so1.to_str().unwrap()).unwrap();

    let mut world = World::new();
    world.init_resource::<Events<AiDecision>>();
    world.init_resource::<Decided>();
    let mut schedule = Schedule::default();
    schedule.add_systems(consumer);

    let metrics = Arc::new(OpaqueMetrics::new());
    let mut chain = OpaqueChain::new(vec![OpaqueNode::Stateful(metrics.clone()), v1.to_node()]);
    println!("[1] 链就绪：OpaqueMetrics + AI 超时 v1（直接依赖宿主）");

    // v1：决策 deadline 宽松 → decided；过期 → 拒
    let mk = |depth: u32, deadline: u64| AiDecision { depth, deadline_ms: deadline, decided: false };
    let mut decided = 0;
    for d in 0..2 {
        let mut ev = mk(d, u64::MAX); // 无限制
        if chain.exec(|e| e.depth, &mut ev).is_ok() {
            world.resource_mut::<Events<AiDecision>>().send(ev);
            decided += 1;
        }
    }
    for d in 2..4 {
        let mut ev = mk(d, now_ms() - 1000); // 过期
        if chain.exec(|e| e.depth, &mut ev).is_ok() {
            world.resource_mut::<Events<AiDecision>>().send(ev);
            decided += 1;
        }
    }
    schedule.run(&mut world);
    println!("[2] v1：4 决策（2 宽松 + 2 过期），{decided} 条通过（期望 2：过期被拒）");
    assert_eq!(decided, 2, "v1 过期决策被拒");

    // 热换 v2（提前 500ms 拒）
    let src_v2 = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/s04_ai_decision_timeout/mw_v2.rs"));
    let so2 = build_plugin_with_deps("large_ai_v2", src_v2, HOST_DEPS, &std::env::temp_dir()).unwrap();
    let v2 = PluginOpaque::load(so2.to_str().unwrap()).unwrap();
    assert!(chain.set(1, v2.to_node()));

    world.resource_mut::<Decided>().0 = 0;
    let mut decided2 = 0;
    for d in 4..6 {
        let mut ev = mk(d, now_ms() + 300); // v1 放行（未过期）/ v2 提前拒
        if chain.exec(|e| e.depth, &mut ev).is_ok() {
            world.resource_mut::<Events<AiDecision>>().send(ev);
            decided2 += 1;
        }
    }
    schedule.run(&mut world);
    println!("[3] 热换 v2（提前500ms）：deadline=now+300（期望 0 通过，v1 会放行）：通过 {decided2}");
    assert_eq!(decided2, 0, "v2 提前 500ms 拒");

    println!("---");
    println!("large S04 AI 决策超时通过：deadline 插件热更 + bevy 事件流 ✓");
}
