//! D6 多插件共存：v1/v2 导出同名符号（proc_mw_abi_version/mw_enter/mw_exit），
//! 验证 RTLD_LOCAL 下互不冲突——同一进程、同一条链同时使用两个插件。
//!
//! 运行：
//!   cargo build -p d6_plugin -p d6_plugin_v2 --release
//!   cargo run --features runtime --release --example d6_multi_plugin

use proc_mw::chain::Chain;
use proc_mw::dispatch::{Ctx, MwError};
use proc_mw::runtime::Plugin;

fn core(ctx: &mut Ctx) -> Result<i32, MwError> {
    Ok(ctx.input + 1)
}

fn plugin_path(name: &str) -> String {
    let manifest = env!("CARGO_MANIFEST_DIR");
    format!("{}/target/release/lib{name}.dylib", manifest)
}

fn main() {
    let p1 = Plugin::load(&plugin_path("d6_plugin")).expect("v1");
    let p2 = Plugin::load(&plugin_path("d6_plugin_v2")).expect("v2");

    // 两条插件在同一条链共存（各自独立 Library，RTLD_LOCAL 无符号冲突）
    let chain = Chain::new(vec![p1.to_node(), p2.to_node()]);
    // enter: 1 → ×10=10 → ×100=1000 → core 1001 → exit 逆序 v2+1000=2001 → v1+100=2101
    let r = chain.exec(core, 1).unwrap();
    assert_eq!(r, 2101);
    println!("多插件共存结果 {r}（预期 2101）✓");

    // 独立验证：两个插件各自行为不变（未受对方符号影响）
    let alone1 = Chain::new(vec![Plugin::load(&plugin_path("d6_plugin")).unwrap().to_node()]);
    let alone2 = Chain::new(vec![Plugin::load(&plugin_path("d6_plugin_v2")).unwrap().to_node()]);
    assert_eq!(alone1.exec(core, 5).unwrap(), 151);
    assert_eq!(alone2.exec(core, 5).unwrap(), 1501);
    println!("各自行为独立（151 / 1501），无符号串扰 ✓");
}
