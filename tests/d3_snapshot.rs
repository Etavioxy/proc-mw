//! D3 动态性·数据/快照层增删 —— 快照行为 + 并发安全测试

use std::sync::{Arc, RwLock};
use std::thread;

use proc_mw::chain::Chain;
use proc_mw::dispatch::{exec, Node};

fn mul3(x: &mut i32) {
    *x *= 3;
}

#[test]
fn add_remove_sequence() {
    let mut c = Chain::new(vec![Node::Add(1), Node::FnPtr(mul3)]);
    assert_eq!(c.exec(5), 18); // (5+1)*3
    c.add(Node::Add(10));
    assert_eq!(c.exec(5), 28); // ((5+1)*3)+10
    c.remove(0);
    assert_eq!(c.exec(5), 25); // (5*3)+10
    c.add(Node::FnPtr(mul3));
    assert_eq!(c.exec(2), 48); // ((2*3)+10)*3
}

#[test]
fn read_path_equals_bare_iteration() {
    let c = Chain::new(vec![Node::Add(1), Node::FnPtr(mul3), Node::Add(10)]);
    let bare: Vec<Node> = vec![Node::Add(1), Node::FnPtr(mul3), Node::Add(10)];
    for &x in &[-10, 0, 5, 1000] {
        assert_eq!(c.exec(x), exec(&bare, x));
    }
}

#[test]
fn concurrent_reads_writes_no_torn() {
    // 4 读者（取一致快照）+ 1 写者（替换快照）
    let shared = Arc::new(RwLock::new(Chain::new(vec![Node::Add(1), Node::Add(2)])));
    let mut handles = Vec::new();
    for _ in 0..4 {
        let s = Arc::clone(&shared);
        handles.push(thread::spawn(move || {
            let mut n = 0u64;
            for _ in 0..200_000 {
                let g = s.read().unwrap();
                let r = g.exec(0);
                assert!(r >= 0, "撕裂读: {}", r);
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
                    g.add(Node::Add((i % 5) as i32));
                } else {
                    let len = g.len();
                    if len > 2 {
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
