//! 实验：插件操作**真实 bevy Entity**（D5/D6 直接依赖路径对生态类型成立）
//!
//! EntityEvent 含 `bevy_ecs::entity::Entity`（真实类型）；插件依赖宿主 + bevy_ecs，
//! 按 Entity.index() 过滤偶数。证明"直接共享类型"兼容真实生态类型。
//!
//! 跑：`cd labs/scales/large && cargo run --release --bin exp_bevy_entity`

use std::sync::Arc;

use bevy_ecs::entity::Entity;
use bevy_ecs::event::{EventReader, Events};
use bevy_ecs::prelude::*;
use bevy_ecs::schedule::Schedule;
use proc_mw::compile::build_plugin_with_deps;
use proc_mw::opaque::{OpaqueChain, OpaqueNode};
use proc_mw::opaque_gov::OpaqueMetrics;
use proc_mw::runtime::PluginOpaque;

use large_service::EntityEvent;

/// 插件依赖：宿主 large_service + **bevy_ecs 本体**（真实类型）
const DEPS: &str = concat!(
    "large_service = { path = \"",
    env!("CARGO_MANIFEST_DIR"),
    "\" }\n",
    "bevy_ecs = \"0.16\""
);

#[derive(Resource, Default)]
struct Kept(u32);

fn consumer(mut r: EventReader<EntityEvent>, mut kept: ResMut<Kept>) {
    for ev in r.read() {
        if ev.kept {
            kept.0 += 1;
        }
    }
}

fn main() {
    let src = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/exp_bevy_entity/mw_v1.rs"));
    let so = build_plugin_with_deps("exp_entity_v1", src, DEPS, &std::env::temp_dir())
        .expect("编译插件（依赖 large_service + bevy_ecs）");
    let plugin = PluginOpaque::load(so.to_str().unwrap()).unwrap();

    let mut world = World::new();
    world.init_resource::<Events<EntityEvent>>();
    world.init_resource::<Kept>();
    let mut schedule = Schedule::default();
    schedule.add_systems(consumer);

    let metrics = Arc::new(OpaqueMetrics::new());
    let chain = OpaqueChain::new(vec![OpaqueNode::Stateful(metrics.clone()), plugin.to_node()]);
    println!("[1] 链就绪：插件依赖宿主 + bevy_ecs（操作真实 Entity）");

    // 实体 0..6：偶数丢弃、奇数保留
    let mut kept = 0;
    for i in 0..6u32 {
        let mut ev = EntityEvent { entity: Entity::from_raw(i), kept: false };
        if chain.exec(|e| e.kept, &mut ev).is_ok() {
            world.resource_mut::<Events<EntityEvent>>().send(ev);
            kept += 1;
        }
    }
    schedule.run(&mut world);
    println!("[2] 实体 0..6：{kept} 条保留（期望 3：奇数 1/3/5），bevy 侧 kept {}", world.resource::<Kept>().0);
    assert_eq!(kept, 3, "偶数实体被真实 Entity 过滤");
    assert_eq!(world.resource::<Kept>().0, 3);
    assert_eq!(metrics.calls(), 6);

    println!("---");
    println!("实验通过：插件操作真实 bevy Entity（直接依赖生态类型）✓");
}
