//! 场景 S06 · 存档校验（large 档，bevy_ecs 事件流 + 校验规则热更）
//!
//! SaveData 经 OpaqueChain（OpaqueMetrics + 校验插件 v1→v2，直接依赖宿主）进
//! bevy `Events<SaveData>`；违规存档被拒（不达 bevy）。
//!
//! 跑：`cd labs/scales/large && cargo run --release --bin s06_save_validate`

use std::sync::Arc;

use bevy_ecs::event::{EventReader, Events};
use bevy_ecs::prelude::*;
use bevy_ecs::schedule::Schedule;
use proc_mw::compile::build_plugin_with_deps;
use proc_mw::opaque::{OpaqueChain, OpaqueNode};
use proc_mw::opaque_gov::OpaqueMetrics;
use proc_mw::runtime::PluginOpaque;

use large_service::SaveData;

const HOST_DEPS: &str = concat!(
    "large_service = { path = \"",
    env!("CARGO_MANIFEST_DIR"),
    "\" }"
);

#[derive(Resource, Default)]
struct Valid(u32);

fn consumer(mut r: EventReader<SaveData>, mut valid: ResMut<Valid>) {
    for ev in r.read() {
        if ev.valid {
            valid.0 += 1;
        }
    }
}

fn main() {
    let src_v1 = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/s06_save_validate/mw_v1.rs"));
    let so1 = build_plugin_with_deps("large_save_v1", src_v1, HOST_DEPS, &std::env::temp_dir()).unwrap();
    let v1 = PluginOpaque::load(so1.to_str().unwrap()).unwrap();

    let mut world = World::new();
    world.init_resource::<Events<SaveData>>();
    world.init_resource::<Valid>();
    let mut schedule = Schedule::default();
    schedule.add_systems(consumer);

    let metrics = Arc::new(OpaqueMetrics::new());
    let mut chain = OpaqueChain::new(vec![OpaqueNode::Stateful(metrics.clone()), v1.to_node()]);
    println!("[1] 链就绪：OpaqueMetrics + 校验 v1（拒负金币，直接依赖宿主）");

    // v1：gold -5（拒）/ 100（放）/ 2_000_000（放，v1 无上限）
    let golds = [-5i64, 100, 2_000_000];
    let mut v1_ok = 0;
    for g in golds {
        let mut ev = SaveData { gold: g, valid: false };
        if chain.exec(|e| e.gold, &mut ev).is_ok() {
            world.resource_mut::<Events<SaveData>>().send(ev);
            v1_ok += 1;
        }
    }
    schedule.run(&mut world);
    println!("[2] v1：存档 {golds:?}，{v1_ok} 条通过（期望 2：负金币被拒）");
    assert_eq!(v1_ok, 2, "v1 拒负金币");

    // 热换 v2（拒负金币 + 超上限）
    let src_v2 = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/s06_save_validate/mw_v2.rs"));
    let so2 = build_plugin_with_deps("large_save_v2", src_v2, HOST_DEPS, &std::env::temp_dir()).unwrap();
    let v2 = PluginOpaque::load(so2.to_str().unwrap()).unwrap();
    assert!(chain.set(1, v2.to_node()));

    world.resource_mut::<Valid>().0 = 0;
    let mut v2_ok = 0;
    for g in golds {
        let mut ev = SaveData { gold: g, valid: false };
        if chain.exec(|e| e.gold, &mut ev).is_ok() {
            world.resource_mut::<Events<SaveData>>().send(ev);
            v2_ok += 1;
        }
    }
    schedule.run(&mut world);
    println!("[3] 热换 v2（超上限拒）：{v2_ok} 条通过（期望 1：仅 100）");
    assert_eq!(v2_ok, 1, "v2 拒负金币 + 超上限");

    println!("---");
    println!("large S06 存档校验通过：校验规则热更 + bevy 事件流 ✓");
}
