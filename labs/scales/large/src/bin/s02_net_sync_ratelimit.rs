//! 场景 S02 · 网络同步限流（large 档，bevy_ecs 事件流 + 限流）
//!
//! 每客户端 NetMsg 经 OpaqueChain（OpaqueMetrics + OpaqueRateLimiter + 同步标记插件，
//! 插件直接依赖宿主）进 bevy `Events<NetMsg>`；超配额被拒（不达 bevy）。
//!
//! 跑：`cd labs/scales/large && cargo run --release --bin s02_net_sync_ratelimit`

use std::sync::Arc;
use std::time::Duration;

use bevy_ecs::event::{EventReader, Events};
use bevy_ecs::prelude::*;
use bevy_ecs::schedule::Schedule;
use proc_mw::compile::build_plugin_with_deps;
use proc_mw::opaque::{OpaqueChain, OpaqueNode};
use proc_mw::opaque_gov::{OpaqueMetrics, OpaqueRateLimiter};
use proc_mw::runtime::PluginOpaque;

use large_service::NetMsg;

const HOST_DEPS: &str = concat!(
    "large_service = { path = \"",
    env!("CARGO_MANIFEST_DIR"),
    "\" }"
);

#[derive(Resource, Default)]
struct Received(u32);

fn consumer(mut r: EventReader<NetMsg>, mut recv: ResMut<Received>) {
    for ev in r.read() {
        if ev.passed {
            recv.0 += 1;
        }
    }
}

fn main() {
    let src = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/s02_net_sync_ratelimit/mw_v1.rs"));
    let so = build_plugin_with_deps("large_sync_v1", src, HOST_DEPS, &std::env::temp_dir()).unwrap();
    let plugin = PluginOpaque::load(so.to_str().unwrap()).unwrap();

    let mut world = World::new();
    world.init_resource::<Events<NetMsg>>();
    world.init_resource::<Received>();
    let mut schedule = Schedule::default();
    schedule.add_systems(consumer);

    let metrics = Arc::new(OpaqueMetrics::new());
    let chain = OpaqueChain::new(vec![
        OpaqueNode::Stateful(metrics.clone()),
        OpaqueNode::Stateful(Arc::new(OpaqueRateLimiter::new(2, Duration::from_secs(10)))),
        plugin.to_node(),
    ]);
    println!("[1] 链就绪：OpaqueMetrics + OpaqueRateLimiter(2) + 同步标记插件（直接依赖宿主）");

    // 5 条 NetMsg：前 2 条通过限流，后 3 条被拒（不达 bevy）
    let mut ok = 0;
    for i in 0..5u32 {
        let mut msg = NetMsg { player_id: 7, kind: 1, payload: i, passed: false };
        if chain.exec(|m| m.payload, &mut msg).is_ok() {
            world.resource_mut::<Events<NetMsg>>().send(msg);
            ok += 1;
        }
    }
    schedule.run(&mut world);
    let recv = world.resource::<Received>().0;
    println!("[2] 限流(2)：5 条 NetMsg 中 {ok} 条通过进 bevy，消费者收到 {recv} 条（期望 2/2）");
    assert_eq!(ok, 2, "前 2 条通过限流");
    assert_eq!(recv, 2, "bevy 消费者只收到通过的 2 条");
    assert_eq!(metrics.calls(), 5, "metrics 计数 5 次尝试");

    println!("---");
    println!("large S02 网络同步限流通过：bevy 事件流 + 限流 + 直接依赖宿主 ✓");
}
