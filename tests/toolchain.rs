//! 工具链域测试：部署环境检查

use proc_mw::compile::toolchain_report;

#[test]
fn toolchain_detected_in_dev() {
    let r = toolchain_report();
    assert!(r.cargo.is_some(), "开发环境应有 cargo");
    assert!(r.rustc.is_some(), "开发环境应有 rustc");
    assert!(r.usable, "运行期编译管线应可用");
    println!("cargo: {:?}", r.cargo);
    println!("rustc: {:?}", r.rustc);
    println!("offline_ready: {}", r.offline_ready);
}
