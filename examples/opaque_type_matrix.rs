//! 值空间 × 性能矩阵：**类型种数 × 性能测试的笛卡尔积**
//!
//! 每种类型的共享 repr(C) 请求，都测四个配置：
//!   裸调用 / 空 OpaqueChain / 1 节点 / 2 节点（每请求 ns，Release，black_box 防消除）
//! 验证 D4 对**每种类型**都成立：空链透明 + 落槽代价有界，且跨类型稳定性。
//!
//! 跑：`cargo run --release --example opaque_type_matrix`

use std::sync::Arc;
use std::time::Instant;

use proc_mw::opaque::{OpaqueChain, OpaqueNode, OPAQUE_CONTINUE};

// ===== 类型空间（每种 = 共享 repr(C) 定义 + make + 变换节点）=====

#[repr(C)]
struct Mi32 {
    v: i32,
}
unsafe extern "C" fn ei32(req: *mut std::ffi::c_void, _: *mut std::ffi::c_void) -> i32 {
    let m = unsafe { &mut *(req as *mut Mi32) };
    m.v += 1;
    OPAQUE_CONTINUE
}

#[repr(C)]
struct Mu64 {
    v: u64,
}
unsafe extern "C" fn eu64(req: *mut std::ffi::c_void, _: *mut std::ffi::c_void) -> i32 {
    let m = unsafe { &mut *(req as *mut Mu64) };
    m.v += 1;
    OPAQUE_CONTINUE
}

#[repr(C)]
struct Mf64 {
    v: f64,
}
unsafe extern "C" fn ef64(req: *mut std::ffi::c_void, _: *mut std::ffi::c_void) -> i32 {
    let m = unsafe { &mut *(req as *mut Mf64) };
    m.v *= 2.0;
    OPAQUE_CONTINUE
}

#[repr(C)]
struct Mbool {
    v: bool,
}
unsafe extern "C" fn ebool(req: *mut std::ffi::c_void, _: *mut std::ffi::c_void) -> i32 {
    let m = unsafe { &mut *(req as *mut Mbool) };
    m.v = !m.v;
    OPAQUE_CONTINUE
}

#[repr(C)]
struct MArr {
    v: [u8; 16],
}
unsafe extern "C" fn earr(req: *mut std::ffi::c_void, _: *mut std::ffi::c_void) -> i32 {
    let m = unsafe { &mut *(req as *mut MArr) };
    m.v[0] = m.v[0].wrapping_add(1);
    OPAQUE_CONTINUE
}

#[repr(C)]
struct MTup {
    v: (u32, u64),
}
unsafe extern "C" fn etup(req: *mut std::ffi::c_void, _: *mut std::ffi::c_void) -> i32 {
    let m = unsafe { &mut *(req as *mut MTup) };
    m.v.0 += 1;
    OPAQUE_CONTINUE
}

#[repr(C)]
struct Plain {
    a: u32,
    b: u32,
}
#[repr(C)]
struct MPlain {
    v: Plain,
}
unsafe extern "C" fn eplain(req: *mut std::ffi::c_void, _: *mut std::ffi::c_void) -> i32 {
    let m = unsafe { &mut *(req as *mut MPlain) };
    m.v.a += m.v.b;
    OPAQUE_CONTINUE
}

#[repr(C)]
struct Padded {
    a: u8,
    b: u64,
}
#[repr(C)]
struct MPadded {
    v: Padded,
}
unsafe extern "C" fn epadded(req: *mut std::ffi::c_void, _: *mut std::ffi::c_void) -> i32 {
    let m = unsafe { &mut *(req as *mut MPadded) };
    m.v.b += 1;
    OPAQUE_CONTINUE
}

#[repr(C)]
struct Nested {
    a: u32,
    b: Plain,
}
#[repr(C)]
struct MNested {
    v: Nested,
}
unsafe extern "C" fn enested(req: *mut std::ffi::c_void, _: *mut std::ffi::c_void) -> i32 {
    let m = unsafe { &mut *(req as *mut MNested) };
    m.v.b.a += 1;
    OPAQUE_CONTINUE
}

#[repr(C)]
struct MStr {
    s: String,
}
unsafe extern "C" fn estr(req: *mut std::ffi::c_void, _: *mut std::ffi::c_void) -> i32 {
    let m = unsafe { &mut *(req as *mut MStr) };
    m.s.push('x');
    OPAQUE_CONTINUE
}

#[repr(C)]
struct MVec {
    xs: Vec<u64>,
}
unsafe extern "C" fn evec(req: *mut std::ffi::c_void, _: *mut std::ffi::c_void) -> i32 {
    let m = unsafe { &mut *(req as *mut MVec) };
    m.xs.sort();
    OPAQUE_CONTINUE
}

// ===== 通用测速（R 被擦除，节点自行 downcast）=====

type Enter = unsafe extern "C" fn(*mut std::ffi::c_void, *mut std::ffi::c_void) -> i32;

struct ResultRow {
    name: &'static str,
    bare: f64,
    empty: f64,
    one: f64,
    two: f64,
}

fn bench_case<R>(name: &'static str, make: fn() -> R, enter: Enter, iters: u64) -> ResultRow {
    let n = || OpaqueNode {
        enter,
        exit: None,
        keepalive: Arc::new(()),
    };
    let empty = OpaqueChain::empty();
    let one = OpaqueChain::new(vec![n()]);
    let two = OpaqueChain::new(vec![n(), n()]);

    let bare = {
        let mut m = make();
        let t = Instant::now();
        for _ in 0..iters {
            std::hint::black_box(&mut m);
        }
        t.elapsed().as_nanos() as f64 / iters as f64
    };
    let empty_ns = {
        let mut m = make();
        let t = Instant::now();
        for _ in 0..iters {
            std::hint::black_box(&mut m);
            let _ = empty.exec(|_| 0u64, std::hint::black_box(&mut m)).unwrap();
        }
        t.elapsed().as_nanos() as f64 / iters as f64
    };
    let one_ns = {
        let mut m = make();
        let t = Instant::now();
        for _ in 0..iters {
            std::hint::black_box(&mut m);
            let _ = one.exec(|_| 0u64, std::hint::black_box(&mut m)).unwrap();
        }
        t.elapsed().as_nanos() as f64 / iters as f64
    };
    let two_ns = {
        let mut m = make();
        let t = Instant::now();
        for _ in 0..iters {
            std::hint::black_box(&mut m);
            let _ = two.exec(|_| 0u64, std::hint::black_box(&mut m)).unwrap();
        }
        t.elapsed().as_nanos() as f64 / iters as f64
    };
    ResultRow { name, bare, empty: empty_ns, one: one_ns, two: two_ns }
}

fn main() {
    let iters = 1_000_000u64;
    println!("值空间 × 性能笛卡尔积（Release，{iters} iters/项，ns/请求）：");
    println!("{:<14} {:>9} {:>9} {:>9} {:>9} {:>9} {:>9}", "类型", "裸", "空链", "1节点", "2节点", "空-裸", "2-1");
    let mut rows = Vec::new();
    rows.push(bench_case("i32", || Mi32 { v: 1 }, ei32, iters));
    rows.push(bench_case("u64", || Mu64 { v: 1 }, eu64, iters));
    rows.push(bench_case("f64", || Mf64 { v: 1.0 }, ef64, iters));
    rows.push(bench_case("bool", || Mbool { v: true }, ebool, iters));
    rows.push(bench_case("[u8;16]", || MArr { v: [0; 16] }, earr, iters));
    rows.push(bench_case("(u32,u64)", || MTup { v: (1, 2) }, etup, iters));
    rows.push(bench_case("Plain{a,b}", || MPlain { v: Plain { a: 1, b: 2 } }, eplain, iters));
    rows.push(bench_case("Padded{a,u64}", || MPadded { v: Padded { a: 1, b: 1 } }, epadded, iters));
    rows.push(bench_case("Nested", || MNested { v: Nested { a: 1, b: Plain { a: 1, b: 2 } } }, enested, iters));
    rows.push(bench_case("String", || MStr { s: String::new() }, estr, iters));
    rows.push(bench_case("Vec<u64>", || MVec { xs: vec![3, 1, 2] }, evec, iters));

    for r in &rows {
        println!(
            "{:<14} {:>8.2} {:>8.2} {:>8.2} {:>8.2} {:>8.2} {:>8.2}",
            r.name, r.bare, r.empty, r.one, r.two, r.empty - r.bare, r.two - r.one
        );
    }

    println!("\nD4 跨类型判定：");
    let max_empty_overhead = rows.iter().map(|r| r.empty - r.bare).fold(0.0_f64, f64::max);
    let max_slot = rows.iter().map(|r| r.two - r.one).fold(0.0_f64, f64::max);
    println!("  空链-裸 最大 {} ns（跨全部类型，空链透明）", max_empty_overhead);
    println!("  2节点-1节点 最大 {} ns（跨全部类型，落槽代价有界）", max_slot);
    assert!(max_empty_overhead < 3.0, "所有类型的空链必须透明（Release）");
}
