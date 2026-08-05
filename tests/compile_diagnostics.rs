//! 错误诊断域测试：编译失败 → 结构化提取错误码/消息/源码行

use proc_mw::compile::{build_plugin_cached, extract_diagnostics};

#[test]
fn extract_diagnostics_parses_stderr() {
    // 快速：直接解析合成 stderr
    let stderr = "error[E0308]: mismatched types\n  --> src/lib.rs:5:17\n  |\n5 |     input.push_str(\"x\");\n";
    let diags = extract_diagnostics(stderr);
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].code, "E0308");
    assert!(diags[0].message.contains("mismatched"), "{}", diags[0].message);
    assert_eq!(diags[0].line, Some(5));
}

#[test]
fn bad_source_gets_structured_diagnostics() {
    // 集成：坏中间件源码 → 真实 cargo 编译失败 → 结构化诊断定位到源码行
    let bad_src = r#"
#[no_mangle]
pub extern "C" fn proc_mw_abi_version() -> i32 { 1 }

#[no_mangle]
pub unsafe extern "C" fn mw_enter(input: *mut i32, _output: *mut i32) -> i32 {
    unsafe { input.push_str("x"); }
    0
}
"#;
    let out_dir = std::env::temp_dir();
    let r = build_plugin_cached("diag_bad", bad_src, &out_dir);
    assert!(r.is_err(), "坏源码必须编译失败");
    let msg = r.unwrap_err();
    assert!(msg.contains("诊断"), "错误应含结构化诊断：{msg}");
    assert!(
        msg.contains("E0599") || msg.contains("E0308"),
        "应提取错误码：{msg}"
    );
    // 应定位到含 push_str 的源码行
    assert!(msg.contains("push_str"), "应附源码对应行：{msg}");
}
