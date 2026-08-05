//! D8 迁移 · 行为等价验证：迁移产物（空链恒等）与原 handler 输出一致

include!("../samples/legacy.rs.mw.rs");

#[test]
fn migrated_handlers_behavior_equivalent() {
    // 原 handle_add: input + 1；迁移后空链恒等 → 结果不变
    assert_eq!(handle_add(5), 6);
    assert_eq!(handle_add(-3), -2);
    // 原 handle_square: input * input（内联计时仍留在核心内，未被剥离）
    assert_eq!(handle_square(5), 25);
    assert_eq!(handle_square(-4), 16);
}

#[test]
fn non_handler_untouched() {
    // helper 不是 handle_* → 未迁移，行为原样
    assert_eq!(helper(3), 6);
}

#[test]
fn cross_cutting_extracted() {
    // D8 深化：计时横切应从核心剥出、生成对应中间件
    let migrated = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/samples/legacy.rs.mw.rs"
    ))
    .unwrap();
    // quote! 渲染带空格（`Node :: FnPtr`），归一化后匹配
    let norm: String = migrated.chars().filter(|c| !c.is_whitespace()).collect();
    assert!(!norm.contains("Instant"), "计时横切应被剥出核心");
    assert!(norm.contains("mw_timing"), "应生成计时中间件 mw_timing");
    assert!(norm.contains("Node::FnPtr(mw_timing)"), "链应填充计时中间件");
}

#[test]
fn multi_file_per_domain() {
    // D8 按域渐进：每个文件独立产物 + 回滚快照
    for name in ["legacy.rs", "legacy2.rs"] {
        let base = format!("{}/samples/{}", env!("CARGO_MANIFEST_DIR"), name);
        assert!(
            std::fs::read_to_string(format!("{base}.mw.rs")).is_ok(),
            "{name}.mw.rs 应存在"
        );
        assert!(
            std::fs::read_to_string(format!("{base}.bak")).is_ok(),
            "{name}.bak 应存在"
        );
    }
    // legacy2（领域 2）的 handler 应被包装
    let mw2 = std::fs::read_to_string(format!(
        "{}/samples/legacy2.rs.mw.rs",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap();
    assert!(mw2.contains("chain_exec"), "legacy2 的 handler 应被包装");
    assert!(mw2.contains("handle_double"), "handle_double 应保留");
}
