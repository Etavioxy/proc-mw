//! 场景 S09 · 行为树并行（large 档，bevy_ecs 事件流 + 多线程并行分支）
//!
//! BehaviorBranch 分支并行经共享 OpaqueChain（OpaqueMetrics + 分支执行插件，直接依赖
//! 宿主）执行（`exec(&self)` 并发安全）；结果聚合进 bevy `Events<BehaviorBranch>`。
//!
//! 跑：`cd labs/scales/large && cargo run --release --bin s09_behavior_tree_parallel`

use std::sync::Arc;

use bevy_ecs::event::{EventReader, Events};
use bevy_ecs::prelude::*;
use bevy_ecs::schedule::Schedule;
use proc_mw::compile::build_plugin_with_deps;
use proc_mw::opaque::{OpaqueChain, OpaqueNode};
use proc_mw::opaque_gov::OpaqueMetrics;
use proc_mw::runtime::PluginOpaque;

use large_service::BehaviorBranch;

const HOST_DEPS: &str = concat!(
    "large_service = { path = \"",
    env!("CARGO_MANIFEST_DIR"),
    "\" }"
);

#[derive(Resource, Default)]
struct Ran(u32);

fn consumer(mut r: EventReader<BehaviorBranch>, mut ran: ResMut<Ran>) {
    for ev in r.read() {
        if ev.ran {
            ran.0 += 1;
        }
    }
}

fn main() {
    let src = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/s09_behavior_tree_parallel/mw_v1.rs"));
    let so = build_plugin_with_deps("large_bt_v1", src, HOST_DEPS, &std::env::temp_dir()).unwrap();
    let plugin = PluginOpaque::load(so.to_str().unwrap()).unwrap();

    let mut world = World::new();
    world.init_resource::<Events<BehaviorBranch>>();
    world.init_resource::<Ran>();
    let mut schedule = Schedule::default();
    schedule.add_systems(consumer);

    let metrics = Arc::new(OpaqueMetrics::new());
    let chain = Arc::new(OpaqueChain::new(vec![OpaqueNode::Stateful(metrics.clone()), plugin.to_node()]));
    println!("[1] 链就绪：OpaqueMetrics + 分支执行插件（共享 Arc，exec(&self) 并发安全）");

    // 8 个分支并行执行（共享链，每线程独立事件）
    let handles: Vec<_> = (0..8u32)
        .map(|b| {
            let chain = Arc::clone(&chain);
            std::thread::spawn(move || {
                let mut ev = BehaviorBranch { branch: b, ran: false };
                chain.exec(|e| e.branch, &mut ev).unwrap();
                ev
            })
        })
        .collect();
    let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    for ev in results {
        world.resource_mut::<Events<BehaviorBranch>>().send(ev);
    }
    schedule.run(&mut world);
    println!("[2] 8 分支并行执行，bevy 消费者收到 {} 条 ran（期望 8）", world.resource::<Ran>().0);
    assert_eq!(world.resource::<Ran>().0, 8, "全部分支并行执行完成");
    assert_eq!(metrics.calls(), 8, "metrics 计数 8 次并行执行");

    println!("---");
    println!("large S09 行为树并行通过：共享链多线程并行 + bevy 事件流 ✓");
}
