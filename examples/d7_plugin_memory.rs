//! D7/L3 边界 · 永不卸载的代价量化：每次热替换加载插件 → 进程 RSS 增长
//!
//! L3：插件永不 unload（防 TLS destructor segfault）。代价 = 每次热替换
//! 新加载的 .so 常驻内存，随替换次数线性增长。量化这条成本曲线。
//!
//! 运行：
//!   cargo build -p d6_plugin --release
//!   cargo run --features runtime --release --example d7_plugin_memory

use proc_mw::runtime::Plugin;

fn rss_kb() -> u64 {
    let out = std::process::Command::new("ps")
        .args(["-o", "rss=", "-p", &std::process::id().to_string()])
        .output()
        .expect("ps");
    String::from_utf8_lossy(&out.stdout).trim().parse().unwrap_or(0)
}

fn main() {
    let manifest = env!("CARGO_MANIFEST_DIR");
    let path = format!("{manifest}/target/release/libd6_plugin.dylib");

    // 基线 RSS
    let base = rss_kb();
    let mut handles: Vec<Plugin> = Vec::new();
    println!("基线 RSS: {base} KB");
    for i in 0..200 {
        handles.push(Plugin::load(&path).expect("加载插件")); // 永不 unload → 全部常驻
        if i % 40 == 39 {
            let rss = rss_kb();
            println!(
                "加载 {} 个插件：RSS {rss} KB，增长 {} KB（+{:.0} KB/插件）",
                i + 1,
                rss.saturating_sub(base),
                (rss.saturating_sub(base)) as f64 / (i + 1) as f64
            );
        }
    }
    let _ = std::hint::black_box(&handles);
}
