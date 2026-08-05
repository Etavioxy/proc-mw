//! D6 运行期加载演示：dlopen 插件 .so → 包装为 Mw → 落进链的 Dyn 槽位
//!
//! 运行：
//!   cargo build -p d6_plugin --release
//!   cargo run --features runtime --release --example d6_runtime_load

use proc_mw::chain::Chain;
use proc_mw::dispatch::{Builtin, Ctx, MwError, Node};
use proc_mw::runtime::Plugin;

fn core_add1(ctx: &mut Ctx) -> Result<i32, MwError> {
    Ok(ctx.input + 1)
}

fn main() {
    // 插件 .so 路径（workspace 共享 target）
    let manifest = env!("CARGO_MANIFEST_DIR");
    let plugin_path = format!("{}/target/release/libd6_plugin.dylib", manifest);
    if !std::path::Path::new(&plugin_path).exists() {
        eprintln!("插件不存在，请先：cargo build -p d6_plugin --release");
        std::process::exit(1);
    }

    // dlopen + ABI 版本校验（D7 边界收口）
    let plugin = Plugin::load(&plugin_path).expect("dlopen 插件失败");
    println!("插件加载成功，ABI v{}", plugin.abi_version());

    // 运行期加载的插件落进 thin 的 Extern 槽位（无状态→thin，不是 Dyn）
    // 与内置 Builtin 共存——D6 是 D1~D5 的超集，且按 D2 落槽
    let mut chain = Chain::new(vec![Node::Builtin(Builtin::Add(1)), plugin.to_node()]);
    // enter: 5 → +1=6 → 插件×10=60 → core +1=61 → exit 插件+100=161
    let r = chain.exec(core_add1, 5).unwrap();
    println!("运行期加载链执行结果: {}（预期 161）", r);
    assert_eq!(r, 161);

    // RCU 快照增删与运行期加载共存：往链里再热加一个内置节点
    chain.add(Node::Builtin(Builtin::Add(1)));
    // enter: 5 → +1=6 → ×10=60 → +1=61 → core +1=62 → exit +100=162
    assert_eq!(chain.exec(core_add1, 5).unwrap(), 162);
    println!("运行期加载 + RCU 热增共存验证通过 ✓");
}
