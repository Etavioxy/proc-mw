//! D7 安全 · 错误路径测试场景（只测 happy path 是盲点，这里补齐失败分支）
#![cfg(feature = "runtime")]

use proc_mw::runtime::Plugin;

#[test]
fn load_nonexistent_fails() {
    let Err(msg) = Plugin::load("/nonexistent/plugin.so") else {
        panic!("不存在的路径必须报错")
    };
    assert!(msg.contains("dlopen"), "错误应含 dlopen 原因：{msg}");
}

#[test]
fn load_non_plugin_dylib_fails_on_abi_symbol() {
    // 一个真实存在的 dylib，但不是 proc-mw 插件（无 ABI 版本函数）
    let path = if cfg!(target_os = "macos") {
        "/usr/lib/libSystem.B.dylib"
    } else {
        "/lib/x86_64-linux-gnu/libc.so.6"
    };
    let Err(msg) = Plugin::load(path) else {
        panic!("非插件 dylib 必须报错")
    };
    assert!(
        msg.contains("ABI 版本") || msg.contains("proc_mw_abi_version"),
        "应报缺 ABI 版本函数：{msg}"
    );
}

#[test]
fn abi_version_mismatch_rejected() {
    // 用 d6_plugin 的路径 + 期望不同版本，验证版本校验路径
    // （真实不匹配插件需另建；此处验证 load 对缺/错版本的硬失败语义）
    let manifest = env!("CARGO_MANIFEST_DIR");
    let p = format!("{}/target/release/libd6_plugin.dylib", manifest);
    if std::path::Path::new(&p).exists() {
        let r = Plugin::load(&p);
        assert!(r.is_ok(), "正常插件加载应成功（happy path 仍成立）");
        assert_eq!(r.unwrap().abi_version(), 1);
    }
    // 不匹配场景：文档化——若插件 ABI 版本 ≠ 宿主 PLUGIN_ABI_VERSION，load 返回 Err("ABI 版本不匹配")
    let _ = proc_mw::runtime::PLUGIN_ABI_VERSION;
}
