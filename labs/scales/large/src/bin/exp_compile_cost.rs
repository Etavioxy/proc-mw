fn main() {
    let src = include_str!("../../s01_input_audit_hotswap/mw_v1.rs");
    let t = std::time::Instant::now();
    let so = proc_mw::compile::build_plugin_with_deps(
        "timing_probe",
        src,
        concat!("large_service = { path = \"", env!("CARGO_MANIFEST_DIR"), "\" }"),
        &std::env::temp_dir(),
    ).unwrap();
    println!("直接依赖插件编译（含 large_service→bevy_ecs）: {:?}", t.elapsed());
    let _ = so;
}
