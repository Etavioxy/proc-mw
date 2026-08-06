//! 场景 S07 · 渲染批次内存限制（large 档，bevy_ecs 事件流 + 上限热更）
//!
//! RenderBatch 经 OpaqueChain（OpaqueMetrics + 上限插件 v1→v2，直接依赖宿主）进
//! bevy `Events<RenderBatch>`；超上限被拒（不达 bevy，内存有界）。
//!
//! 跑：`cd labs/scales/large && cargo run --release --bin s07_render_memory_limit`

use std::sync::Arc;

use bevy_ecs::event::{EventReader, Events};
use bevy_ecs::prelude::*;
use bevy_ecs::schedule::Schedule;
use proc_mw::compile::build_plugin_with_deps;
use proc_mw::opaque::{OpaqueChain, OpaqueNode};
use proc_mw::opaque_gov::OpaqueMetrics;
use proc_mw::runtime::PluginOpaque;

use large_service::RenderBatch;

const HOST_DEPS: &str = concat!(
    "large_service = { path = \"",
    env!("CARGO_MANIFEST_DIR"),
    "\" }"
);

#[derive(Resource, Default)]
struct Accepted(u32);

fn consumer(mut r: EventReader<RenderBatch>, mut acc: ResMut<Accepted>) {
    for ev in r.read() {
        if ev.accepted {
            acc.0 += 1;
        }
    }
}

fn main() {
    let src_v1 = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/s07_render_memory_limit/mw_v1.rs"));
    let so1 = build_plugin_with_deps("large_render_v1", src_v1, HOST_DEPS, &std::env::temp_dir()).unwrap();
    let v1 = PluginOpaque::load(so1.to_str().unwrap()).unwrap();

    let mut world = World::new();
    world.init_resource::<Events<RenderBatch>>();
    world.init_resource::<Accepted>();
    let mut schedule = Schedule::default();
    schedule.add_systems(consumer);

    let metrics = Arc::new(OpaqueMetrics::new());
    let mut chain = OpaqueChain::new(vec![OpaqueNode::Stateful(metrics.clone()), v1.to_node()]);
    println!("[1] 链就绪：OpaqueMetrics + 内存上限 v1(>1000 拒)");

    // v1：批次 200 / 1500 → 200 通过、1500 拒
    let counts = [200u32, 1500];
    let mut v1_ok = 0;
    for c in counts {
        let mut ev = RenderBatch { sprite_count: c, accepted: false };
        if chain.exec(|e| e.sprite_count, &mut ev).is_ok() {
            world.resource_mut::<Events<RenderBatch>>().send(ev);
            v1_ok += 1;
        }
    }
    schedule.run(&mut world);
    println!("[2] v1(>1000 拒)：批次 {counts:?}，{v1_ok} 通过（期望 1）");
    assert_eq!(v1_ok, 1, "v1 拒超上限批次");

    // 热换 v2（>500 拒）
    let src_v2 = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/s07_render_memory_limit/mw_v2.rs"));
    let so2 = build_plugin_with_deps("large_render_v2", src_v2, HOST_DEPS, &std::env::temp_dir()).unwrap();
    let v2 = PluginOpaque::load(so2.to_str().unwrap()).unwrap();
    assert!(chain.set(1, v2.to_node()));

    let mut v2_ok = 0;
    for c in [200u32, 800] {
        let mut ev = RenderBatch { sprite_count: c, accepted: false };
        if chain.exec(|e| e.sprite_count, &mut ev).is_ok() {
            world.resource_mut::<Events<RenderBatch>>().send(ev);
            v2_ok += 1;
        }
    }
    schedule.run(&mut world);
    println!("[3] 热换 v2(>500 拒)：批次 [200, 800]，{v2_ok} 通过（期望 1）");
    assert_eq!(v2_ok, 1, "v2 拒 800 批次");

    println!("---");
    println!("large S07 渲染内存限制通过：上限热更 + bevy 事件流 ✓");
}
