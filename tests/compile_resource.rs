//! 资源管理域测试：临时 crate 清理（防泄漏）+ 缓存预算淘汰

use proc_mw::compile::{build_plugin_cached, cache_cleanup};

fn mw_src(offset: i32) -> String {
    format!(
        r#"#[no_mangle]
pub extern "C" fn proc_mw_abi_version() -> i32 {{ 1 }}
#[no_mangle]
pub unsafe extern "C" fn mw_enter(input: *mut i32, _o: *mut i32) -> i32 {{
    unsafe {{ *input += {offset}; }}
    0
}}"#
    )
}

#[test]
fn temp_crates_cleaned_after_cache() {
    let out_dir = std::env::temp_dir().join(format!("proc_mw_res_test_{}", std::process::id()));
    let so = build_plugin_cached("res_test", &mw_src(1), &out_dir).unwrap();
    let cache_dir = out_dir.join("proc_mw_compile_cache");
    assert!(so.starts_with(&cache_dir), "产物应在缓存目录");
    // 临时 crate（{out_dir}/res_test_*）应已被清理
    let leaked = std::fs::read_dir(&out_dir)
        .unwrap()
        .filter(|e| e.as_ref().unwrap().file_name().to_string_lossy().starts_with("res_test_"))
        .count();
    assert_eq!(leaked, 0, "临时 crate 应清理，泄漏 {} 个", leaked);
}

#[test]
fn temp_crates_cleaned_on_failure() {
    // 失败路径：编译错误也应清理临时 crate（防泄漏）
    let out_dir = std::env::temp_dir().join(format!("proc_mw_res_fail_{}", std::process::id()));
    let bad_src = "fn not_plugin() { let x: i32 = \"str\"; }"; // 编译错误
    let r = build_plugin_cached("fail_test", bad_src, &out_dir);
    assert!(r.is_err(), "坏源码必须编译失败");
    let leaked = std::fs::read_dir(&out_dir)
        .unwrap()
        .filter(|e| e.as_ref().unwrap().file_name().to_string_lossy().starts_with("fail_test_"))
        .count();
    assert_eq!(leaked, 0, "失败路径临时 crate 应清理，泄漏 {leaked} 个");
}

#[test]
fn cache_cleanup_evicts_to_budget() {
    let out_dir = std::env::temp_dir().join(format!("proc_mw_res_clean_{}", std::process::id()));
    for i in 0..3 {
        build_plugin_cached("cleanup_test", &mw_src(i + 1), &out_dir).unwrap();
    }
    let before = std::fs::read_dir(out_dir.join("proc_mw_compile_cache")).unwrap().count();
    assert_eq!(before, 3, "3 个不同源码 → 3 条缓存");
    // 预算 0 → 淘汰到只剩 1 个
    let removed = cache_cleanup(&out_dir, 0);
    assert!(removed >= 2, "应淘汰 ≥2 个，移除了 {removed}");
    let after = std::fs::read_dir(out_dir.join("proc_mw_compile_cache")).unwrap().count();
    assert!(after <= 1, "缓存应收敛到 ≤1，剩 {after}");
}
