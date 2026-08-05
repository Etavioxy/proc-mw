//! D3 动态性 · 测试场景：快照增删 + 并发（生产形状洋葱执行）

use std::sync::{Arc, RwLock};
use std::thread;

use proc_mw::chain::Chain;
use proc_mw::dispatch::{Builtin, Ctx, MwError, Node};

fn core_add1(ctx: &mut Ctx) -> Result<i32, MwError> {
    Ok(ctx.input + 1)
}

#[test]
fn add_remove_sequence_onion() {
    // [Add(1)]：enter 5→6 → core 7（Add 仅进入侧，无 exit 双重计数）
    let mut c = Chain::new(vec![Node::Builtin(Builtin::Add(1))]);
    assert_eq!(c.exec(core_add1, 5).unwrap(), 7);
    c.add(Node::Builtin(Builtin::RejectNegative));
    // 加 RejectNegative 后负输入被拒
    assert_eq!(
        c.exec(core_add1, -5),
        Err(MwError::Rejected("negative input"))
    );
    c.remove(0); // 移除 Add(1)，剩 RejectNegative
    assert_eq!(c.len(), 1);
    // 正输入：enter 5 → core 6 → exit(无) → 6
    assert_eq!(c.exec(core_add1, 5).unwrap(), 6);
}

#[test]
fn read_path_consistent_after_swap() {
    let mut c = Chain::new(vec![Node::Builtin(Builtin::Add(1))]);
    let before = c.exec(core_add1, 5).unwrap();
    c.add(Node::Builtin(Builtin::Add(10)));
    let after = c.exec(core_add1, 5).unwrap();
    // 快照替换前后各自一致（Add 仅进入侧）
    assert_eq!(before, 7); // 5→6 → core 7
    assert_eq!(after, 17); // 5→6→16 → core 17
}

#[test]
fn concurrent_reads_writes_no_torn() {
    let shared = Arc::new(RwLock::new(Chain::new(vec![Node::Builtin(Builtin::Add(1))])));
    let mut handles = Vec::new();
    for _ in 0..4 {
        let s = Arc::clone(&shared);
        handles.push(thread::spawn(move || {
            let mut n = 0u64;
            for _ in 0..200_000 {
                let g = s.read().unwrap();
                let r = g.exec(core_add1, 0);
                assert!(r.is_ok(), "读路径必须成功");
                n += 1;
            }
            n
        }));
    }
    {
        let s = Arc::clone(&shared);
        handles.push(thread::spawn(move || {
            let mut writes = 0u64;
            for i in 0..20_000u32 {
                let mut g = s.write().unwrap();
                if i % 2 == 0 {
                    g.add(Node::Builtin(Builtin::Add((i % 5) as i32)));
                } else {
                    let len = g.len();
                    if len > 1 {
                        g.remove(len - 1);
                    }
                }
                writes += 1;
            }
            writes
        }));
    }
    for h in handles {
        h.join().unwrap();
    }
}
