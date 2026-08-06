//! 场景 S03 · 碰撞事件去重（large 档，bevy_ecs 事件流 + 去重插件）
//!
//! CollisionEvent 经 OpaqueChain（OpaqueMetrics + 去重插件，插件内 HashSet 记已见对，
//! **任意 Rust**）进 bevy `Events<CollisionEvent>`；重复碰撞被去重（不达 bevy）。
//!
//! 跑：`cd labs/scales/large && cargo run --release --bin s03_collision_dedup`

use std::sync::Arc;

use bevy_ecs::event::{EventReader, Events};
use bevy_ecs::prelude::*;
use bevy_ecs::schedule::Schedule;
use proc_mw::compile::build_plugin_with_deps;
use proc_mw::opaque::{OpaqueChain, OpaqueNode};
use proc_mw::opaque_gov::OpaqueMetrics;
use proc_mw::runtime::PluginOpaque;

use large_service::CollisionEvent;

const HOST_DEPS: &str = concat!(
    "large_service = { path = \"",
    env!("CARGO_MANIFEST_DIR"),
    "\" }"
);

#[derive(Resource, Default)]
struct Resolved(u32);

fn consumer(mut r: EventReader<CollisionEvent>, mut resolved: ResMut<Resolved>) {
    for ev in r.read() {
        if ev.resolved {
            resolved.0 += 1;
        }
    }
}

fn main() {
    let src = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/s03_collision_dedup/mw_v1.rs"));
    let so = build_plugin_with_deps("large_dedup_v1", src, HOST_DEPS, &std::env::temp_dir()).unwrap();
    let plugin = PluginOpaque::load(so.to_str().unwrap()).unwrap();

    let mut world = World::new();
    world.init_resource::<Events<CollisionEvent>>();
    world.init_resource::<Resolved>();
    let mut schedule = Schedule::default();
    schedule.add_systems(consumer);

    let metrics = Arc::new(OpaqueMetrics::new());
    let chain = OpaqueChain::new(vec![OpaqueNode::Stateful(metrics.clone()), plugin.to_node()]);
    println!("[1] 链就绪：OpaqueMetrics + 去重插件（插件内 HashSet，任意 Rust）");

    // 碰撞对：(1,2), (2,1) 同对（归一化去重），(3,4), (1,2) 重复
    let pairs = [(1, 2), (2, 1), (3, 4), (1, 2)];
    let mut ok = 0;
    for (a, b) in pairs {
        let mut ev = CollisionEvent { a, b, resolved: false };
        if chain.exec(|e| e.resolved, &mut ev).is_ok() {
            world.resource_mut::<Events<CollisionEvent>>().send(ev);
            ok += 1;
        }
    }
    schedule.run(&mut world);
    let resolved = world.resource::<Resolved>().0;
    println!("[2] 4 碰撞对（含重复）：{ok} 条通过（期望 2：仅 2 个唯一碰撞对），bevy 侧 resolved {resolved} 条");
    assert_eq!(ok, 2, "4 对碰撞去重后仅 2 唯一：(1,2)/(2,1) 归一化同对 + (3,4)");
    assert_eq!(resolved, 2, "bevy 消费者只收到去重后的事件");
    assert_eq!(metrics.calls(), 4, "metrics 计数 4 次尝试");

    println!("---");
    println!("large S03 碰撞去重通过：插件内 HashSet 去重 + 直接依赖宿主 ✓");
}
