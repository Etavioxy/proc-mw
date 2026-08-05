//! D1 表达层·零成本抽象 —— 行为测试（汇编等价见 docs/results/d1_expression.md）

use proc_mw::{build_pipeline, direct_bare, through_pipeline};

const SAMPLES: &[i32] = &[-10, 0, 5, 1000];

#[test]
fn behavior_consistent_both_paths() {
    // 双路径对多样本结果一致，且核心语义保持 x+1
    for &x in SAMPLES {
        assert_eq!(through_pipeline(x), direct_bare(x));
        assert_eq!(through_pipeline(x), x + 1);
    }
}

#[test]
fn release_pipeline_is_zst() {
    #[cfg(not(debug_assertions))]
    {
        let p = build_pipeline();
        assert_eq!(std::mem::size_of_val(&p), 0, "Release 必须退化为 ZST");
    }
}

#[test]
fn debug_pipeline_carries_state() {
    #[cfg(debug_assertions)]
    {
        let p = build_pipeline();
        assert!(std::mem::size_of_val(&p) > 0, "Debug 必须携带中间件状态");
    }
}
