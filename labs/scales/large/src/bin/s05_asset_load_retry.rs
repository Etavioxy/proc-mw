//! 场景 S05 · 资源加载重试（large 档，bevy_ecs 事件流 + exec_retry）
//!
//! AssetLoad 经 OpaqueChain（OpaqueMetrics + Flaky 瞬时失败 + 加载插件，直接依赖宿主）
//! 的 `exec_retry` 重试至成功；成功（loaded）进 bevy `Events<AssetLoad>`。
//!
//! 跑：`cd labs/scales/large && cargo run --release --bin s05_asset_load_retry`

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use bevy_ecs::event::{EventReader, Events};
use bevy_ecs::prelude::*;
use bevy_ecs::schedule::Schedule;
use proc_mw::compile::build_plugin_with_deps;
use proc_mw::opaque::{OpaqueChain, OpaqueMw, OpaqueNode};
use proc_mw::opaque_gov::OpaqueMetrics;
use proc_mw::runtime::PluginOpaque;

use large_service::AssetLoad;

const HOST_DEPS: &str = concat!(
    "large_service = { path = \"",
    env!("CARGO_MANIFEST_DIR"),
    "\" }"
);

/// 瞬时加载失败：前 `fail_left` 次返回码 2
struct Flaky {
    fail_left: Arc<AtomicUsize>,
}
impl OpaqueMw for Flaky {
    fn enter(&self, _req: *mut std::ffi::c_void) -> i32 {
        if self.fail_left.load(Ordering::SeqCst) > 0 {
            self.fail_left.fetch_sub(1, Ordering::SeqCst);
            2
        } else {
            0
        }
    }
}

#[derive(Resource, Default)]
struct Loaded(u32);

fn consumer(mut r: EventReader<AssetLoad>, mut loaded: ResMut<Loaded>) {
    for ev in r.read() {
        if ev.loaded {
            loaded.0 += 1;
        }
    }
}

fn main() {
    let src = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/s05_asset_load_retry/mw_v1.rs"));
    let so = build_plugin_with_deps("large_load_v1", src, HOST_DEPS, &std::env::temp_dir()).unwrap();
    let plugin = PluginOpaque::load(so.to_str().unwrap()).unwrap();

    let mut world = World::new();
    world.init_resource::<Events<AssetLoad>>();
    world.init_resource::<Loaded>();
    let mut schedule = Schedule::default();
    schedule.add_systems(consumer);

    let metrics = Arc::new(OpaqueMetrics::new());
    let chain = OpaqueChain::new(vec![
        OpaqueNode::Stateful(metrics.clone()),
        OpaqueNode::Stateful(Arc::new(Flaky { fail_left: Arc::new(AtomicUsize::new(2)) })),
        plugin.to_node(),
    ]);
    println!("[1] 链就绪：OpaqueMetrics + Flaky(前2失败) + 加载插件，exec_retry(5)");

    // 3 个资源：前 2 次瞬时失败 + retry 5 → 全部加载成功进 bevy
    let mut ok = 0;
    for i in 0..3u32 {
        let mut ev = AssetLoad { path: format!("/assets/{i}"), loaded: false };
        if chain.exec_retry(|e| e.loaded, &mut ev, 5).is_ok() {
            world.resource_mut::<Events<AssetLoad>>().send(ev);
            ok += 1;
        }
    }
    schedule.run(&mut world);
    println!("[2] 瞬时失败+retry5：3 资源全部加载（期望 3），bevy 侧 loaded {}", world.resource::<Loaded>().0);
    assert_eq!(ok, 3, "重试后全部加载成功");
    assert_eq!(world.resource::<Loaded>().0, 3);

    // 耗尽：新 Flaky(3) + retry 1 → 失败不进 bevy
    let metrics2 = Arc::new(OpaqueMetrics::new());
    let chain2 = OpaqueChain::new(vec![
        OpaqueNode::Stateful(metrics2.clone()),
        OpaqueNode::Stateful(Arc::new(Flaky { fail_left: Arc::new(AtomicUsize::new(3)) })),
        plugin.to_node(),
    ]);
    let mut ev2 = AssetLoad { path: "/assets/x".into(), loaded: false };
    let r = chain2.exec_retry(|e| e.loaded, &mut ev2, 1);
    println!("[3] 失败3次+retry1（期望 Err，不进 bevy）：{r:?}");
    assert!(r.is_err(), "重试耗尽透传错误");

    println!("---");
    println!("large S05 资源加载重试通过：exec_retry + 直接依赖宿主 + bevy 事件流 ✓");
}
