//! 并发编译安全测试：多线程同时 build_plugin_cached
//! 验证：唯一 crate 目录（原子计数）+ 原子缓存写（tmp+rename）无冲突

use std::thread;

use proc_mw::compile::build_plugin_cached;

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
fn concurrent_compilation_safe() {
    let out_dir = std::env::temp_dir().join(format!("proc_mw_cc_{}", std::process::id()));
    let handles: Vec<_> = (0..4)
        .map(|i| {
            let out = out_dir.clone();
            thread::spawn(move || {
                build_plugin_cached(&format!("cc_{i}"), &mw_src(i + 1), &out)
            })
        })
        .collect();
    for (i, h) in handles.into_iter().enumerate() {
        let r = h.join().unwrap();
        assert!(r.is_ok(), "并发编译 cc_{i} 必须成功: {:?}", r.err());
    }
    // 4 个不同源码 → 缓存 4 条产物（无覆盖/无丢失）
    let count = std::fs::read_dir(out_dir.join("proc_mw_compile_cache")).unwrap().count();
    assert_eq!(count, 4, "并发编译后缓存应有 4 条产物");
}

#[test]
fn build_plugin_with_deps_cached_works() {
    use proc_mw::compile::build_plugin_with_deps_cached;
    let out_dir = std::env::temp_dir().join(format!("proc_mw_wdc_{}", std::process::id()));
    let so1 = build_plugin_with_deps_cached("wdc_probe", &mw_src(1), "", &out_dir).unwrap();
    let so2 = build_plugin_with_deps_cached("wdc_probe", &mw_src(1), "", &out_dir).unwrap();
    assert_eq!(so1, so2, "同源码+deps → 缓存命中同产物");
    let so3 = build_plugin_with_deps_cached("wdc_probe", &mw_src(2), "", &out_dir).unwrap();
    assert_ne!(so1, so3, "不同源码 → 不同产物");
    // 产物可加载（直接依赖路径的缓存化）
    let p = proc_mw::runtime::PluginOpaque::load(so1.to_str().unwrap()).unwrap();
    assert_eq!(p.abi_version(), 1);
}

#[test]
fn plugin_target_cleanup_caps_shared_target() {
    use proc_mw::compile::plugin_target_cleanup;
    let out_dir = std::env::temp_dir().join(format!("proc_mw_ptc_{}", std::process::id()));
    // 构建一个插件（产生共享 target）
    build_plugin_cached("ptc_probe", &mw_src(1), &out_dir).unwrap();
    let target = out_dir.join("proc_mw_plugin_target");
    assert!(target.exists(), "共享 target 存在");
    // 低上限 → 清理（整删，返回 1）
    let cleaned = plugin_target_cleanup(&out_dir, 0);
    assert_eq!(cleaned, 1, "超限清理");
    assert!(!target.exists(), "超限后 target 被删");
    // 后续可重建（新源码 → 缓存 miss → 重新构建）
    build_plugin_cached("ptc_probe2", &mw_src(2), &out_dir).unwrap();
    assert!(out_dir.join("proc_mw_plugin_target").exists(), "清理后可重建");
}

#[test]
fn pipeline_stats_tracks_builds_and_hits() {
    use proc_mw::compile::pipeline_stats;
    let out_dir = std::env::temp_dir().join(format!("proc_mw_stats_{}", std::process::id()));
    // 首次编译 → miss（总请求增、命中不变）
    build_plugin_cached("stats_probe", &mw_src(1), &out_dir).unwrap();
    let (total1, hits1) = pipeline_stats();
    assert!(total1 >= 1);
    // 同源码再编译 → 命中（命中增）
    build_plugin_cached("stats_probe", &mw_src(1), &out_dir).unwrap();
    let (total2, hits2) = pipeline_stats();
    assert!(hits2 > hits1, "同源码第二次命中缓存");
    assert!(total2 > total1);
}

#[test]
fn concurrent_same_source_single_cache_entry() {
    // 多线程编译相同源码 → 缓存只应有 1 条（原子写防重复）
    let out_dir = std::env::temp_dir().join(format!("proc_mw_cc_same_{}", std::process::id()));
    let handles: Vec<_> = (0..4)
        .map(|_| {
            let out = out_dir.clone();
            thread::spawn(move || build_plugin_cached("same", &mw_src(1), &out))
        })
        .collect();
    for h in handles {
        assert!(h.join().unwrap().is_ok());
    }
    let count = std::fs::read_dir(out_dir.join("proc_mw_compile_cache")).unwrap().count();
    assert_eq!(count, 1, "相同源码并发编译 → 缓存应只有 1 条（原子写）");
}
