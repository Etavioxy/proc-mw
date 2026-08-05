//! 实验 01 · D1 表达层 · 零成本抽象
//!
//! 验证三个承诺（对应用户约束 D1）：
//! 1. **业务核心零污染** —— `Add` 不含任何 cfg / println，它只是 `x + 1`
//! 2. **装配表达式语义直白** —— 遮蔽 + cfg，Release 下中间件在词法层不存在
//! 3. **Release 机器码级等价** —— 包装路径与裸调用路径生成完全一致的汇编
//!
//! 多重测试约定（用户 Goal：必须多重测试）：
//! - 多样本：x ∈ {-10, 0, 5, 1000}
//! - 双构建：Debug 与 Release 各跑一遍，行为断言相同
//! - 内存足迹：Release 必须 ZST(0B)，Debug 携带中间件状态
//! - 机器码：`cargo rustc --release -- --emit=asm` 比对两个符号

// ===== 协议 =====
trait Proc {
    fn exec(&self, x: i32) -> i32;
}

// ===== 纯业务核心：零污染（D1 承诺 1）=====
#[derive(Clone, Copy)]
struct Add;
impl Proc for Add {
    fn exec(&self, x: i32) -> i32 {
        x + 1 // 不含 cfg，不含 println —— 绝对干净
    }
}

// ===== 中间件：日志壳（独立于业务之外，带状态以便观察内存足迹）=====
struct Log<T> {
    inner: T,
    tag: &'static str,
}
impl<T: Proc> Proc for Log<T> {
    fn exec(&self, x: i32) -> i32 {
        println!("[{:?}] enter: {}", self.tag, x);
        let y = self.inner.exec(x);
        println!("[{:?}] exit: {}", self.tag, y);
        y
    }
}

// ===== 装配：遮蔽 + cfg（D1 承诺 2）
// Release 下 `let p = Log{...}` 这行在词法层消失 → p 直接是 Add
#[inline(always)]
fn build_pipeline() -> impl Proc {
    #[cfg(debug_assertions)]
    let p = Log { inner: Add, tag: "debug" };
    #[cfg(not(debug_assertions))]
    let p = Add;
    p
}

// ===== 对照函数：都 inline(never)，保证在汇编中作为独立符号出现 =====
#[inline(never)]
fn through_pipeline(x: i32) -> i32 {
    build_pipeline().exec(x)
}

#[inline(never)]
fn direct_bare(x: i32) -> i32 {
    x + 1
}

// ===== 多路测试样本 =====
const SAMPLES: &[i32] = &[-10, 0, 5, 1000];

fn main() {
    // ---- 测试 1：行为一致性（多样本，双路径）----
    for &x in SAMPLES {
        let wrapped = through_pipeline(x);
        let bare = direct_bare(x);
        assert_eq!(wrapped, bare, "样本 x={} 双路径结果必须一致", x);
        assert_eq!(wrapped, x + 1, "样本 x={} 核心语义必须保持 x+1", x);
    }
    println!("[T1] 行为一致性：{} 个样本双路径全部一致 ✓", SAMPLES.len());

    // ---- 测试 2：内存足迹（Release 零占用 / Debug 携带状态）----
    #[cfg(debug_assertions)]
    {
        let p = build_pipeline();
        let sz = std::mem::size_of_val(&p);
        assert!(sz > 0, "Debug 必须携带中间件状态");
        println!("[T2] Debug  内存足迹 = {}B（携带 &str tag）✓", sz);
    }
    #[cfg(not(debug_assertions))]
    {
        let p = build_pipeline();
        let sz = std::mem::size_of_val(&p);
        assert_eq!(sz, 0, "Release 必须退化为 ZST（零占用）");
        println!("[T2] Release 内存足迹 = {}B（ZST，零占用）✓", sz);
    }

    // ---- 测试 3：构建模式标记（Debug 有日志 / Release 无）----
    #[cfg(debug_assertions)]
    println!("[T3] 当前构建 = DEBUG：日志中间件生效（上方已打印 enter/exit）✓");
    #[cfg(not(debug_assertions))]
    println!("[T3] 当前构建 = RELEASE：日志中间件已被 cfg 词法层剥离（无任何 log 输出）✓");

    // ---- 结论 ----
    println!("---");
    println!("PASS 全部测试。Assembly 等价验证：见 RESULT.md（cargo rustc --release -- --emit=asm）");
}
