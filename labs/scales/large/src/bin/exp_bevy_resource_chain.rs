//! 实验：链作为 **bevy Resource** 由 Schedule 系统访问（真实 bevy 集成模式）
//!
//! 此前 large 场景在 main 手动跑链；真实 bevy 应用是系统处理。本实验把 OpaqueChain
//! 存为 bevy Resource，生产系统读资源跑链、结果写入资源计数器。
//!
//! 跑：`cd labs/scales/large && cargo run --release --bin exp_bevy_resource_chain`

use std::sync::Arc;

use bevy_ecs::prelude::*;
use bevy_ecs::schedule::Schedule;
use proc_mw::opaque::{OpaqueChain, OpaqueNode};
use proc_mw::opaque_gov::OpaqueMetrics;

use large_service::InputEvent;

/// 链作为 bevy Resource（OpaqueChain: Send+Sync）
#[derive(Resource)]
struct MwChain(OpaqueChain);

#[derive(Resource, Default)]
struct ChainRuns(u32);

/// bevy 生产系统：读链资源，跑链（变换 InputEvent），结果写入资源
fn producer(chain: Res<MwChain>, mut runs: ResMut<ChainRuns>) {
    let mut ev = InputEvent { key: 1, pos: (0.0, 0.0), audited: false };
    chain.0.exec(|e| e.key, &mut ev).unwrap();
    runs.0 += 1;
}

fn main() {
    let metrics = Arc::new(OpaqueMetrics::new());
    let chain = OpaqueChain::new(vec![OpaqueNode::Stateful(metrics.clone())]);

    let mut world = World::new();
    world.insert_resource(MwChain(chain));
    world.init_resource::<ChainRuns>();
    let mut schedule = Schedule::default();
    schedule.add_systems(producer);

    // 跑 2 帧：每帧生产系统经链资源跑 1 次
    schedule.run(&mut world);
    schedule.run(&mut world);
    println!("[1] bevy Schedule 系统经链资源跑 {} 次（期望 2）", world.resource::<ChainRuns>().0);
    assert_eq!(world.resource::<ChainRuns>().0, 2, "链资源被系统访问");
    assert_eq!(metrics.calls(), 2, "链资源 metrics 计数");
    println!("---");
    println!("实验通过：链作为 bevy Resource 由 Schedule 系统访问 ✓");
}
