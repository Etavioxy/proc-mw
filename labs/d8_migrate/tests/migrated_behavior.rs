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
