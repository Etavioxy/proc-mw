//! 值表示空间研究：中间件状态如何承载（ZST/结构体/容器/值对象/闭包）
//! 每项验证行为 + 尺寸，作为 docs/survey.md 的代码证据。

use std::sync::Arc;

use proc_mw::dispatch::{Ctx, Flow, Mw, MwError};

fn core_add1(ctx: &mut Ctx) -> Result<i32, MwError> {
    Ok(ctx.input + 1)
}

// ---- 1. ZST：零大小类型（无状态）----
struct ZstMw;
impl Mw for ZstMw {
    fn enter(&self, _ctx: &mut Ctx) -> Result<Flow, MwError> {
        Ok(Flow::Continue)
    }
    fn exit(&self, _ctx: &mut Ctx) {}
    fn box_clone(&self) -> Box<dyn Mw> {
        Box::new(ZstMw)
    }
}

#[test]
fn zst_mw_size_zero() {
    assert_eq!(std::mem::size_of::<ZstMw>(), 0, "ZST 无状态，0B");
}

// ---- 2. 一般结构体：内联状态 ----
struct OffsetMw {
    n: i32,
}
impl Mw for OffsetMw {
    fn enter(&self, ctx: &mut Ctx) -> Result<Flow, MwError> {
        ctx.input += self.n;
        Ok(Flow::Continue)
    }
    fn exit(&self, _ctx: &mut Ctx) {}
    fn box_clone(&self) -> Box<dyn Mw> {
        Box::new(OffsetMw { n: self.n })
    }
}

#[test]
fn struct_mw_inline_state() {
    assert_eq!(std::mem::size_of::<OffsetMw>(), 4, "结构体内联 i32 状态 4B");
    let mw = OffsetMw { n: 10 };
    let mut ctx = Ctx::new(5);
    mw.enter(&mut ctx).unwrap();
    assert_eq!(ctx.input, 15);
}

// ---- 3. 容器：Box 堆上状态 ----
struct BoxedMw {
    data: Box<Vec<i32>>,
}
impl Mw for BoxedMw {
    fn enter(&self, ctx: &mut Ctx) -> Result<Flow, MwError> {
        for k in self.data.iter() {
            ctx.input += k;
        }
        Ok(Flow::Continue)
    }
    fn exit(&self, _ctx: &mut Ctx) {}
    fn box_clone(&self) -> Box<dyn Mw> {
        Box::new(BoxedMw {
            data: self.data.clone(),
        })
    }
}

#[test]
fn box_container_mw() {
    // Box<Vec<i32>>：指针(8B) 指向堆上变长状态
    assert_eq!(
        std::mem::size_of::<BoxedMw>(),
        8,
        "Box 容器：指针 8B，数据在堆"
    );
    let mw = BoxedMw {
        data: Box::new(vec![1, 2, 3]),
    };
    let mut ctx = Ctx::new(0);
    mw.enter(&mut ctx).unwrap();
    assert_eq!(ctx.input, 6);
}

// ---- 4. 容器：Arc 共享状态 ----
struct ArcMw {
    counter: Arc<std::sync::atomic::AtomicI32>,
}
impl Mw for ArcMw {
    fn enter(&self, ctx: &mut Ctx) -> Result<Flow, MwError> {
        ctx.input += self.counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Ok(Flow::Continue)
    }
    fn exit(&self, _ctx: &mut Ctx) {}
    fn box_clone(&self) -> Box<dyn Mw> {
        Box::new(ArcMw {
            counter: Arc::clone(&self.counter),
        })
    }
}

#[test]
fn arc_shared_state_mw() {
    assert_eq!(
        std::mem::size_of::<ArcMw>(),
        8,
        "Arc 共享状态：8B，原子引用计数"
    );
    let shared = Arc::new(std::sync::atomic::AtomicI32::new(10));
    let mw = ArcMw {
        counter: Arc::clone(&shared),
    };
    let mut ctx = Ctx::new(0);
    mw.enter(&mut ctx).unwrap();
    assert_eq!(ctx.input, 10, "共享计数状态跨调用可见");
}

// ---- 5. 值对象：Copy 枚举（配置即值）----
#[derive(Clone, Copy)]
enum PolicyMw {
    Allow,
    Deny, // 短路
}
impl Mw for PolicyMw {
    fn enter(&self, ctx: &mut Ctx) -> Result<Flow, MwError> {
        match self {
            PolicyMw::Allow => Ok(Flow::Continue),
            PolicyMw::Deny => Err(MwError::Rejected("deny")),
        }
    }
    fn exit(&self, _ctx: &mut Ctx) {}
    fn box_clone(&self) -> Box<dyn Mw> {
        Box::new(*self)
    }
}

#[test]
fn value_object_mw() {
    assert_eq!(std::mem::size_of::<PolicyMw>(), 1, "Copy 值对象：仅判别式 1B");
    let allow = PolicyMw::Allow;
    let deny = PolicyMw::Deny;
    let mut ctx = Ctx::new(5);
    assert_eq!(allow.enter(&mut ctx), Ok(Flow::Continue));
    assert_eq!(deny.enter(&mut ctx), Err(MwError::Rejected("deny")));
}

// ---- 6. 闭包：捕获状态 ----
#[test]
fn closure_state_mw() {
    // 无捕获闭包 → fn 指针（8B）；捕获闭包 → 需装箱或 Fn trait
    let captured = 7i32;
    let with_capture = move |ctx: &mut Ctx| -> Result<Flow, MwError> {
        ctx.input += captured;
        Ok(Flow::Continue)
    };
    // 捕获闭包不是 fn 指针：尺寸 = 捕获量
    assert!(std::mem::size_of_val(&with_capture) > 0);
    let mut ctx = Ctx::new(1);
    with_capture(&mut ctx).unwrap();
    assert_eq!(ctx.input, 8);
    // 无捕获闭包可转 fn 指针（8B）
    fn no_capture(ctx: &mut Ctx) -> Result<Flow, MwError> {
        ctx.input += 1;
        Ok(Flow::Continue)
    }
    let f = no_capture as fn(&mut Ctx) -> Result<Flow, MwError>;
    assert_eq!(std::mem::size_of_val(&f), 8);
}

// ---- 7. 结论辅助：给调研文档用 ----
#[test]
fn summarize_sizes() {
    println!(
        "值表示尺寸：ZST=0 | struct=4 | Box<Vec>=8 | Arc=8 | 值对象enum=1 | 无捕获fn=8"
    );
    let _ = core_add1; // 防止未用
}
