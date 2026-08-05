//! D7 安全 · 子进程沙箱演示
//!
//! 坏插件（panic abort）只杀子进程，宿主存活；可检测崩溃并重启。
//!
//! 运行：
//!   cargo build --bin mw_exec --features runtime --release
//!   cargo build -p d6_plugin -p d6_plugin_bad --release
//!   cargo run --features runtime --release --example d7_sandbox

use proc_mw::sandbox::Sandbox;

fn main() {
    let manifest = env!("CARGO_MANIFEST_DIR");
    let exec = format!("{manifest}/target/release/mw_exec");
    let good = format!("{manifest}/target/release/libd6_plugin.dylib");
    let bad = format!("{manifest}/target/release/libd6_plugin_bad.dylib");

    // 1) 好插件在沙箱中正常工作
    let sb = Sandbox::spawn(std::path::Path::new(&exec), std::path::Path::new(&good)).expect("启动沙箱");
    let r1 = sb.run(5).unwrap();
    assert_eq!(r1, 50, "好插件 ×10：5 → 50（沙箱内）");
    println!("好插件沙箱内: run(5) = {r1} ✓");

    // 2) 坏插件（panic abort）→ 只杀子进程，宿主存活
    let sb_bad = Sandbox::spawn(std::path::Path::new(&exec), std::path::Path::new(&bad)).expect("启动坏沙箱");
    let r2 = sb_bad.run(5);
    assert!(r2.is_err(), "坏插件必须让子进程崩溃（EOF）");
    println!("坏插件沙箱: run(5) = Err（子进程崩溃，宿主存活）✓");

    // 3) 宿主仍存活：好沙箱继续工作
    let r3 = sb.run(7).unwrap();
    assert_eq!(r3, 70);
    println!("宿主存活验证: 好沙箱 run(7) = {r3} ✓");

    // 4) 崩溃后重启沙箱
    sb_bad.restart().unwrap();
    let r4 = sb_bad.run(5);
    // 重启后仍是坏插件（再次崩溃），但 restart 机制本身可用
    println!("崩溃后重启: run(5) = {:?}（restart 机制工作）", r4);

    println!("D7 子进程沙箱：隔离崩溃 + 宿主存活 + 可重启 ✓");
}
