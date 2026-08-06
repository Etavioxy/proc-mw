//! 场景 S12 · 世界状态逻辑热更（large 档旗舰，bevy_ecs 世界更新 + 掉落表热更）
//!
//! WorldState 经 OpaqueChain（OpaqueMetrics + 世界逻辑插件 v1→v2，直接依赖宿主）进
//! bevy `Events<WorldState>`。掉落表 1% → 3% 热更，世界更新不停（bevy Schedule 持续跑）。
//!
//! 跑：`cd labs/scales/large && cargo run --release --bin s12_world_state_hotswap`

use std::sync::Arc;

use bevy_ecs::event::{EventReader, Events};
use bevy_ecs::prelude::*;
use bevy_ecs::schedule::Schedule;
use proc_mw::compile::build_plugin_with_deps;
use proc_mw::opaque::{OpaqueChain, OpaqueNode};
use proc_mw::opaque_gov::OpaqueMetrics;
use proc_mw::runtime::PluginOpaque;

use large_service::WorldState;

const HOST_DEPS: &str = concat!(
    "large_service = { path = \"",
    env!("CARGO_MANIFEST_DIR"),
    "\" }"
);

#[derive(Resource, Default)]
struct Rates(Vec<f32>);

fn consumer(mut r: EventReader<WorldState>, mut rates: ResMut<Rates>) {
    for ev in r.read() {
        if ev.updated {
            rates.0.push(ev.drop_rate);
        }
    }
}

fn main() {
    let src_v1 = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/s12_world_state_hotswap/mw_v1.rs"));
    let so1 = build_plugin_with_deps("large_world_v1", src_v1, HOST_DEPS, &std::env::temp_dir()).unwrap();
    let v1 = PluginOpaque::load(so1.to_str().unwrap()).unwrap();

    let mut world = World::new();
    world.init_resource::<Events<WorldState>>();
    world.init_resource::<Rates>();
    let mut schedule = Schedule::default();
    schedule.add_systems(consumer);

    let metrics = Arc::new(OpaqueMetrics::new());
    let mut chain = OpaqueChain::new(vec![OpaqueNode::Stateful(metrics.clone()), v1.to_node()]);
    println!("[1] 链就绪：OpaqueMetrics + 世界逻辑 v1(掉落 1%)");

    // 世界更新（bevy Schedule 持续跑）：v1 掉落率 1%
    for i in 0..3u32 {
        let mut ev = WorldState { drop_rate: 0.0, updated: false };
        chain.exec(|e| e.updated, &mut ev).unwrap();
        world.resource_mut::<Events<WorldState>>().send(ev);
    }
    schedule.run(&mut world);
    let rates_v1 = world.resource::<Rates>().0.clone();
    println!("[2] v1 世界更新 3 帧，掉落率（期望 [0.01,0.01,0.01]）：{rates_v1:?}");
    assert!(rates_v1.iter().all(|r| (*r - 0.01).abs() < 1e-6), "v1 掉落率 1%");

    // 热换 v2(3%)——世界不停
    let src_v2 = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/s12_world_state_hotswap/mw_v2.rs"));
    let so2 = build_plugin_with_deps("large_world_v2", src_v2, HOST_DEPS, &std::env::temp_dir()).unwrap();
    let v2 = PluginOpaque::load(so2.to_str().unwrap()).unwrap();
    assert!(chain.set(1, v2.to_node()));
    println!("[3] 热替换：世界逻辑 v1(1%) → v2(3%)，bevy Schedule 不停");

    for i in 0..3u32 {
        let mut ev = WorldState { drop_rate: 0.0, updated: false };
        chain.exec(|e| e.updated, &mut ev).unwrap();
        world.resource_mut::<Events<WorldState>>().send(ev);
    }
    schedule.run(&mut world);
    let rates_v2 = world.resource::<Rates>().0.clone();
    let v2_only = &rates_v2[3..];
    println!("[4] v2 世界更新 3 帧，掉落率（期望 [0.03,0.03,0.03]）：{v2_only:?}");
    assert!(v2_only.iter().all(|r| (*r - 0.03).abs() < 1e-6), "v2 掉落率 3%（活动生效）");

    println!("---");
    println!("large S12 世界状态热更通过：掉落表 1%→3% 热更，世界更新不停 ✓");
}
