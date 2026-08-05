//! D6 扩展形态 · 不停机热替换演示
//!
//! 同一进程、同一条链：加载插件 v1（×10）→ 执行 → 加载 v2（×100）→
//! `Chain::set` 快照内替换节点 → 执行 → 行为变化，无需重启。
//!
//! 运行：
//!   cargo build -p d6_plugin --release -p d6_plugin_v2 --release
//!   cargo run --features runtime --release --example d6_hot_replace

use proc_mw::chain::Chain;
use proc_mw::dispatch::{Ctx, MwError, Node};
use proc_mw::runtime::Plugin;

fn core(ctx: &mut Ctx) -> Result<i32, MwError> {
    Ok(ctx.input + 1)
}

fn plugin_path(name: &str) -> String {
    let manifest = env!("CARGO_MANIFEST_DIR");
    format!("{}/target/release/lib{name}.dylib", manifest)
}

fn main() {
    let p1_path = plugin_path("d6_plugin");
    let p2_path = plugin_path("d6_plugin_v2");
    assert!(std::path::Path::new(&p1_path).exists(), "构建 v1 插件");
    assert!(std::path::Path::new(&p2_path).exists(), "构建 v2 插件");

    // 首次加载 v1
    let p1 = Plugin::load(&p1_path).expect("加载 v1");
    let mut chain = Chain::new(vec![p1.to_node()]);
    // v1: 5 ×10=50 → core 51 → exit +100=151
    let r1 = chain.exec(core, 5).unwrap();
    assert_eq!(r1, 151);
    println!("[v1] 链结果 {r1}（×10/+100，预期 151）");

    // ---- 不停机热替换：加载 v2，快照内替换节点 ----
    let p2 = Plugin::load(&p2_path).expect("加载 v2");
    assert!(chain.set(0, p2.to_node()), "热替换节点");
    // v2: 5 ×100=500 → core 501 → exit +1000=1501
    let r2 = chain.exec(core, 5).unwrap();
    assert_eq!(r2, 1501);
    println!("[v2] 同链热替换后结果 {r2}（×100/+1000，预期 1501）");

    // 回滚演示：换回 v1（加载新句柄，永不 unload 旧句柄）
    let p1b = Plugin::load(&p1_path).expect("回滚加载 v1");
    chain.set(0, p1b.to_node());
    let r3 = chain.exec(core, 5).unwrap();
    assert_eq!(r3, 151);
    println!("[v1] 回滚后结果 {r3}（回到 151）");

    println!("D6 不停机热替换 + 回滚验证通过 ✓");
}
