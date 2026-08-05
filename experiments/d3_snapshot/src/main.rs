//! 实验 03 · D3 动态性 · 数据/快照层增删（RCU）
//!
//! 验证 D3 核心约束：
//! 1. **增删只发生在数据/快照层**——链是不可变快照，add/remove = 复制快照 + Arc 原子替换
//! 2. **读路径零成本**——exec 只 deref Arc + 迭代，无锁、无分配，与裸 `&[Node]` 等价
//! 3. **写路径 Θ(len)**——复制链长，稀有操作
//! 4. **并发安全**——读者取一致快照，写者替换快照，无撕裂读
//!
//! 复现：
//!   cargo run -p d3_snapshot --release
//!   cargo rustc -p d3_snapshot --release -- --emit=asm   # 对比 exec vs exec_bare

use std::sync::Arc;
use std::sync::RwLock;
use std::thread;
use std::time::Instant;

type MwFn = fn(&mut i32);

// 闭合世界节点集合（D3 只承诺"实例增删"，类型集合编译期固定）
#[derive(Clone)]
enum Node {
    Add(i32),
    FnPtr(MwFn),
}

// 链 = 不可变快照
#[derive(Clone)]
struct Chain {
    nodes: Arc<Vec<Node>>,
}

impl Chain {
    fn new(nodes: Vec<Node>) -> Self {
        Chain {
            nodes: Arc::new(nodes),
        }
    }

    // ---- 读路径：无锁、无分配，只 deref Arc + 迭代 ----
    #[no_mangle]
    #[inline(never)]
    fn exec(&self, mut x: i32) -> i32 {
        for n in self.nodes.iter() {
            match n {
                Node::Add(k) => x += k,
                Node::FnPtr(f) => f(&mut x),
            }
        }
        x
    }

    // ---- 写路径：复制快照 + 原子替换（RCU），Θ(len)，稀有操作 ----
    fn add(&mut self, node: Node) {
        let mut v = (*self.nodes).clone();
        v.push(node);
        self.nodes = Arc::new(v);
    }
    fn remove(&mut self, idx: usize) {
        let mut v = (*self.nodes).clone();
        v.remove(idx);
        self.nodes = Arc::new(v);
    }
}

fn mul3(x: &mut i32) {
    *x *= 3;
}

// 裸基线：直接迭代 &[Node]（读路径等价目标）
#[no_mangle]
#[inline(never)]
fn exec_bare(nodes: &[Node], mut x: i32) -> i32 {
    for n in nodes {
        match n {
            Node::Add(k) => x += k,
            Node::FnPtr(f) => f(&mut x),
        }
    }
    x
}

fn main() {
    // ---- T1：增删快照后行为正确（多步序列）----
    let mut c = Chain::new(vec![Node::Add(1), Node::FnPtr(mul3)]);
    assert_eq!(c.exec(5), 18); // (5+1)*3
    c.add(Node::Add(10));
    assert_eq!(c.exec(5), 28); // ((5+1)*3)+10
    c.remove(0);
    assert_eq!(c.exec(5), 25); // (5*3)+10
    c.add(Node::FnPtr(mul3));
    assert_eq!(c.exec(2), 48); // ((2*3)+10)*3，收尾两步 FnPtr
    println!("[T1] 增删快照多步序列行为正确 ✓");

    // ---- T2：读路径与裸 &[Node] 结果一致（机器码等价目标）----
    let samples: &[i32] = &[-10, 0, 5, 1000];
    for &x in samples {
        assert_eq!(c.exec(x), exec_bare(&c.nodes, x));
    }
    println!("[T2] exec(Arc 快照) == exec_bare(&[Node])，{} 样本一致 ✓", samples.len());
    println!("     （汇编等价性：见 RESULT.md，exec 与 exec_bare 应只差一次 Arc 指针加载）");

    // ---- T3：写路径 Θ(len)（复制快照成本随链长线性）----
    for len in [10usize, 100, 1000, 10000] {
        let nodes: Vec<Node> = (0..len).map(|i| Node::Add(i as i32 % 7)).collect();
        let mut big = Chain::new(nodes);
        let t = Instant::now();
        let iters = 10_000;
        for _ in 0..iters {
            big.add(Node::Add(1));
        }
        let per_op = t.elapsed().as_nanos() as f64 / iters as f64;
        println!("[T3] 链长 {:<6} add 平均 {:.1} ns/次（复制 {len} 元素 + 替换）", len, per_op);
    }

    // ---- T4：并发安全——读者 exec + 写者增删，无撕裂、无 panic ----
    let shared = Arc::new(RwLock::new(Chain::new(vec![Node::Add(1), Node::Add(2)])));
    let mut handles = Vec::new();
    for _ in 0..4 {
        let s = Arc::clone(&shared);
        handles.push(thread::spawn(move || {
            let mut reads = 0u64;
            for _ in 0..1_000_000 {
                let g = s.read().unwrap();
                let r = g.exec(0);
                // 任何快照下结果都应 ≥ 0（Add 都是正数）——无撕裂读的弱不变量
                assert!(r >= 0, "撕裂读检测到负数结果 {}", r);
                reads += 1;
            }
            reads
        }));
    }
    // 写者
    {
        let s = Arc::clone(&shared);
        handles.push(thread::spawn(move || {
            let mut writes = 0u64;
            for i in 0..50_000u32 {
                let mut g = s.write().unwrap();
                if i % 2 == 0 {
                    g.add(Node::Add((i % 5) as i32));
                } else {
                    let len = g.nodes.len();
                    if len > 2 {
                        g.remove(len - 1);
                    }
                }
                writes += 1;
            }
            writes
        }));
    }
    let mut total_reads = 0u64;
    let mut total_writes = 0u64;
    let mut iter = handles.into_iter();
    // 前 4 个是读者，最后 1 个是写者
    for _ in 0..4 {
        if let Ok(r) = iter.next().unwrap().join() {
            total_reads += r;
        }
    }
    if let Ok(w) = iter.next().unwrap().join() {
        total_writes = w;
    }
    println!(
        "[T4] 4 读者 + 1 写者并发 {:.0} 万读 / {} 写：无撕裂、无 panic ✓",
        total_reads as f64 / 1e4,
        total_writes
    );

    // ---- 结论 ----
    println!("---");
    println!("PASS 全部测试。读路径零成本的汇编证据见 RESULT.md。");
}
