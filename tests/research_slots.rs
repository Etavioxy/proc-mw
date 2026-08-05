//! 分派机制空间研究：位标记 / 索引注册表 / 整链预编译
//! 每项验证行为 + 代价，作为 docs/survey.md 的代码证据。

use proc_mw::dispatch::{Ctx, Flow, MwError, Mw};

// ===== 机制 1：位标记（开关型中间件）=====
// 一组固定中间件各对应一个开关位，一次 bit test 决定是否应用。
#[derive(Clone, Copy)]
struct Switches(u64);
impl Switches {
    fn on(&mut self, i: u8) {
        self.0 |= 1 << i;
    }
    fn off(&mut self, i: u8) {
        self.0 &= !(1 << i);
    }
    fn enabled(&self, i: u8) -> bool {
        self.0 & (1 << i) != 0
    }
}

// 一组固定中间件（各对应开关位）
fn mw_add1(ctx: &mut Ctx) {
    ctx.input += 1;
}
fn mw_double(ctx: &mut Ctx) {
    ctx.input *= 2;
}
fn mw_add10(ctx: &mut Ctx) {
    ctx.input += 10;
}

#[test]
fn bitflag_switches_enable_disable() {
    let mut sw = Switches(0);
    sw.on(0); // 只开 mw_add1
    let mut ctx = Ctx::new(5);
    if sw.enabled(0) {
        mw_add1(&mut ctx);
    }
    if sw.enabled(1) {
        mw_double(&mut ctx);
    }
    assert_eq!(ctx.input, 6, "只应用了开着的 mw0");
    sw.on(1);
    let mut ctx2 = Ctx::new(5);
    if sw.enabled(0) {
        mw_add1(&mut ctx2);
    }
    if sw.enabled(1) {
        mw_double(&mut ctx2);
    }
    assert_eq!(ctx2.input, 12, "(5+1)*2");
    // 尺寸：一位开关成本 = u64 掩码
    assert_eq!(std::mem::size_of::<Switches>(), 8);
}

// ===== 机制 2：索引注册表（半开放：ID → 处理函数）=====
type RegFn = fn(&mut Ctx) -> Result<Flow, MwError>;

fn reg_add1(ctx: &mut Ctx) -> Result<Flow, MwError> {
    ctx.input += 1;
    Ok(Flow::Continue)
}
fn reg_reject(ctx: &mut Ctx) -> Result<Flow, MwError> {
    Err(MwError::Rejected("registry deny"))
}

const K: usize = 16;
struct Registry {
    table: [Option<RegFn>; K],
}
impl Registry {
    fn new() -> Self {
        Registry {
            table: [None; K],
        }
    }
    fn register(&mut self, id: usize, f: RegFn) {
        self.table[id] = Some(f);
    }
    fn dispatch(&self, id: usize, ctx: &mut Ctx) -> Result<Flow, MwError> {
        match self.table[id] {
            Some(f) => f(ctx),
            None => Ok(Flow::Continue),
        }
    }
}

#[test]
fn indexed_registry_dispatch() {
    let mut reg = Registry::new();
    reg.register(3, reg_add1);
    reg.register(9, reg_reject);
    let mut ctx = Ctx::new(5);
    reg.dispatch(3, &mut ctx).unwrap();
    assert_eq!(ctx.input, 6);
    assert_eq!(reg.dispatch(9, &mut ctx), Err(MwError::Rejected("registry deny")));
    // 未注册 ID → 直通
    assert_eq!(reg.dispatch(0, &mut ctx), Ok(Flow::Continue));
    // 尺寸：K 槽位表 = 16 × 8B（半开放，新类型 = 编译期注册）
    assert_eq!(std::mem::size_of::<Registry>(), 128);
}

// ===== 机制 3：整链预编译（chain-as-function）=====
// 宏把一条固定链编译成单一函数，per-handler = 一次直调（LLVM 全内联）。
macro_rules! chain_fn {
    ($name:ident, [$( $mw:expr ),*]) => {
        #[inline(never)]
        fn $name(x: i32) -> Result<i32, MwError> {
            let mut ctx = Ctx::new(x);
            $(
                match $mw.enter(&mut ctx) {
                    Ok(Flow::Continue) => {}
                    Ok(Flow::Break) => return Err(MwError::Halted),
                    Err(e) => return Err(e),
                }
            )*
            ctx.output = ctx.input + 1; // 核心
            $(
                $mw.exit(&mut ctx);
            )*
            Ok(ctx.output)
        }
    };
}

struct AddMw;
impl Mw for AddMw {
    fn enter(&self, ctx: &mut Ctx) -> Result<Flow, MwError> {
        ctx.input += 1;
        Ok(Flow::Continue)
    }
    fn exit(&self, _ctx: &mut Ctx) {}
    fn box_clone(&self) -> Box<dyn Mw> {
        Box::new(AddMw)
    }
}
struct CapMw;
impl Mw for CapMw {
    fn enter(&self, ctx: &mut Ctx) -> Result<Flow, MwError> {
        if ctx.input > 100 {
            ctx.input = 100;
        }
        Ok(Flow::Continue)
    }
    fn exit(&self, _ctx: &mut Ctx) {}
    fn box_clone(&self) -> Box<dyn Mw> {
        Box::new(CapMw)
    }
}

chain_fn!(chain_standard, [AddMw, CapMw]);
chain_fn!(chain_light, [AddMw]);

#[test]
fn chain_as_function() {
    // 整链预编译：5 → +1=6 → cap(不触发) → core +1=7
    assert_eq!(chain_standard(5).unwrap(), 7);
    // cap 触发：200 → +1=201 → cap 100 → core 101
    assert_eq!(chain_standard(200).unwrap(), 101);
    assert_eq!(chain_light(5).unwrap(), 7);
}

// ===== 机制 4：泛型单态化（已否决——为何不行的证据）=====
// 每核心一个泛型实例 → 编译成本随核心数线性，二进制膨胀
//（实测见 labs/d5_incremental/RESULT.md：1.5× 二进制 + 全量增量重编）
#[test]
fn generic_per_core_rejected() {
    // 本文档只记录结论，实证在 labs/d5_incremental
    // 对比：整链预编译（机制 3）只对"有限几种标准链形状"编译一次，
    // 泛型却对"每核心"编译一次——前者有界，后者线性。
    let _ = std::hint::black_box(chain_standard);
}
