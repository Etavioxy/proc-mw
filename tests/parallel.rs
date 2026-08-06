//! 并行原语测试：exec_parallel 并发处理 N 输入聚合

use std::time::{Duration, Instant};

use proc_mw::chain::Chain;
use proc_mw::dispatch::{Builtin, Ctx, MwError, Node};

fn core(ctx: &mut Ctx) -> Result<i32, MwError> {
    Ok(ctx.input + 1)
}

fn slow_core(ctx: &mut Ctx) -> Result<i32, MwError> {
    std::thread::sleep(Duration::from_millis(20)); // 模拟慢核心
    Ok(ctx.input + 1)
}

#[test]
fn parallel_aggregates_results() {
    let chain = Chain::new(vec![Node::Builtin(Builtin::Add(1))]);
    let results = chain.exec_parallel(core, vec![1, 2, 3, 4]).unwrap();
    assert_eq!(results, vec![3, 4, 5, 6], "1→+1=2→core 3 ...");
}

#[test]
fn parallel_is_faster_than_serial_for_slow_core() {
    // 慢核心（20ms）：并行应显著快于串行
    let chain = Chain::new(vec![]);
    let inputs = vec![1; 4];
    let t_par = Instant::now();
    chain.exec_parallel(slow_core, inputs.clone()).unwrap();
    let par_ms = t_par.elapsed().as_millis();

    let t_ser = Instant::now();
    for _ in &inputs {
        let _ = chain.exec(slow_core, 1).unwrap();
    }
    let ser_ms = t_ser.elapsed().as_millis();
    println!("串行 {ser_ms}ms vs 并行 {par_ms}ms");
    // 并行应明显快于串行（4×20ms 串行 vs ~20ms 并行）
    assert!(par_ms < ser_ms, "并行必须快于串行");
}

#[test]
fn parallel_worker_panic_surfaces() {
    let chain = Chain::new(vec![]);
    let r = chain.exec_parallel(|_| -> Result<i32, MwError> { panic!("worker bug") }, vec![1]);
    assert_eq!(r, Err(MwError::Rejected("worker panicked")));
}
