//! 值空间 × 正确性矩阵：任意类型经 OpaqueChain 的完整验证（D2 类型种数）
//!
//! 每类：宿主共享 repr(C) 定义 + 变换节点 + 断言。堆/容器类型（String/Vec/Box/
//! Option/Result）附**运行期编译插件全链路**——这是核心目的"编译任意 Rust 代码
//! 操作任意类型"的直接证据。
//!
//! 类型轴 × 性能轴（笛卡尔积）见 examples/opaque_type_matrix.rs。

use std::sync::Arc;

use proc_mw::opaque::{OpaqueChain, OpaqueNode, OPAQUE_CONTINUE};

fn node(f: unsafe extern "C" fn(*mut std::ffi::c_void, *mut std::ffi::c_void) -> i32) -> OpaqueNode {
    OpaqueNode {
        enter: f,
        exit: None,
        keepalive: Arc::new(()),
    }
}

// ===== 标量 =====

#[repr(C)]
#[derive(Clone, Copy, PartialEq, Debug)]
struct Mi32 {
    v: i32,
}
unsafe extern "C" fn mi32_enter(req: *mut std::ffi::c_void, _: *mut std::ffi::c_void) -> i32 {
    let m = unsafe { &mut *(req as *mut Mi32) };
    m.v += 1;
    OPAQUE_CONTINUE
}

#[repr(C)]
#[derive(Clone, Copy, PartialEq, Debug)]
struct Mu64 {
    v: u64,
}
unsafe extern "C" fn mu64_enter(req: *mut std::ffi::c_void, _: *mut std::ffi::c_void) -> i32 {
    let m = unsafe { &mut *(req as *mut Mu64) };
    m.v = m.v.wrapping_mul(3).wrapping_add(1);
    OPAQUE_CONTINUE
}

#[repr(C)]
#[derive(Clone, Copy, PartialEq, Debug)]
struct Mf64 {
    v: f64,
}
unsafe extern "C" fn mf64_enter(req: *mut std::ffi::c_void, _: *mut std::ffi::c_void) -> i32 {
    let m = unsafe { &mut *(req as *mut Mf64) };
    m.v *= 2.0;
    OPAQUE_CONTINUE
}

#[repr(C)]
#[derive(Clone, Copy, PartialEq, Debug)]
struct Mbool {
    v: bool,
}
unsafe extern "C" fn mbool_enter(req: *mut std::ffi::c_void, _: *mut std::ffi::c_void) -> i32 {
    let m = unsafe { &mut *(req as *mut Mbool) };
    m.v = !m.v;
    OPAQUE_CONTINUE
}

#[repr(C)]
#[derive(Clone, Copy, PartialEq, Debug)]
struct Mchar {
    v: char,
}
unsafe extern "C" fn mchar_enter(req: *mut std::ffi::c_void, _: *mut std::ffi::c_void) -> i32 {
    let m = unsafe { &mut *(req as *mut Mchar) };
    m.v = m.v.to_ascii_uppercase();
    OPAQUE_CONTINUE
}

// ===== 数组 / 元组 =====

#[repr(C)]
#[derive(Clone, Copy, PartialEq, Debug)]
struct MArr {
    v: [u8; 16],
}
unsafe extern "C" fn marr_enter(req: *mut std::ffi::c_void, _: *mut std::ffi::c_void) -> i32 {
    let m = unsafe { &mut *(req as *mut MArr) };
    m.v[0] = m.v[0].wrapping_add(1);
    OPAQUE_CONTINUE
}

#[repr(C)]
#[derive(Clone, Copy, PartialEq, Debug)]
struct MTup {
    v: (u32, u64),
}
unsafe extern "C" fn mtup_enter(req: *mut std::ffi::c_void, _: *mut std::ffi::c_void) -> i32 {
    let m = unsafe { &mut *(req as *mut MTup) };
    m.v.0 += 1;
    m.v.1 += 2;
    OPAQUE_CONTINUE
}

// ===== 枚举（repr(C) 判别式布局稳定）=====

#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Debug)]
enum Kind {
    A = 0,
    B = 1,
}
#[repr(C)]
#[derive(Clone, Copy, PartialEq, Debug)]
struct MEnum {
    v: Kind,
}
unsafe extern "C" fn menum_enter(req: *mut std::ffi::c_void, _: *mut std::ffi::c_void) -> i32 {
    let m = unsafe { &mut *(req as *mut MEnum) };
    m.v = match m.v {
        Kind::A => Kind::B,
        Kind::B => Kind::A,
    };
    OPAQUE_CONTINUE
}

// ===== 结构：普通 / 嵌套 / 带填充 =====

#[repr(C)]
#[derive(Clone, Copy, PartialEq, Debug)]
struct Plain {
    a: u32,
    b: u32,
}
#[repr(C)]
#[derive(Clone, Copy, PartialEq, Debug)]
struct MPlain {
    v: Plain,
}
unsafe extern "C" fn mplain_enter(req: *mut std::ffi::c_void, _: *mut std::ffi::c_void) -> i32 {
    let m = unsafe { &mut *(req as *mut MPlain) };
    m.v.a += m.v.b;
    OPAQUE_CONTINUE
}

#[repr(C)]
#[derive(Clone, Copy, PartialEq, Debug)]
struct Nested {
    a: u32,
    b: Plain,
}
#[repr(C)]
#[derive(Clone, Copy, PartialEq, Debug)]
struct MNested {
    v: Nested,
}
unsafe extern "C" fn mnested_enter(req: *mut std::ffi::c_void, _: *mut std::ffi::c_void) -> i32 {
    let m = unsafe { &mut *(req as *mut MNested) };
    m.v.b.a += 1;
    OPAQUE_CONTINUE
}

#[repr(C)]
#[derive(Clone, Copy, PartialEq, Debug)]
struct Padded {
    a: u8,
    b: u64, // 引入 padding：a 后 7 字节空洞——布局守卫关键
}
#[repr(C)]
#[derive(Clone, Copy, PartialEq, Debug)]
struct MPadded {
    v: Padded,
}
const _: () = assert!(std::mem::size_of::<Padded>() == 16);
const _: () = assert!(std::mem::offset_of!(Padded, b) == 8);
unsafe extern "C" fn mpadded_enter(req: *mut std::ffi::c_void, _: *mut std::ffi::c_void) -> i32 {
    let m = unsafe { &mut *(req as *mut MPadded) };
    m.v.b += 1;
    OPAQUE_CONTINUE
}

// ===== 堆类型：插件全链路（核心目的展示）=====

/// String::push —— 运行期编译插件操作堆字符串字段
#[test]
fn plugin_with_string_push() {
    let src = r#"
#[repr(C)]
pub struct Msg { pub s: String }
#[no_mangle] pub extern "C" fn proc_mw_abi_version() -> i32 { 1 }
#[no_mangle] pub unsafe extern "C" fn mw_enter(req: *mut std::ffi::c_void, _resp: *mut std::ffi::c_void) -> i32 {
    let m = unsafe { &mut *(req as *mut Msg) };
    m.s.push('!');           // String::push
    m.s.push_str("HOOT");    // String::push_str
    0
}
"#;
    let so = proc_mw::compile::build_plugin_cached("types_string", src, &std::env::temp_dir()).unwrap();
    let p = proc_mw::runtime::PluginOpaque::load(so.to_str().unwrap()).unwrap();
    #[repr(C)]
    struct Msg {
        s: String,
    }
    let mut m = Msg { s: String::from("caw") };
    let chain = OpaqueChain::new(vec![p.to_node()]);
    chain.exec(|_| {}, &mut m).unwrap();
    assert_eq!(m.s, "caw!HOOT", "运行期编译插件对 String 字段调用 push/push_str");
}

/// Vec::sort —— 运行期编译插件操作容器字段
#[test]
fn plugin_with_vec_sort() {
    let src = r#"
#[repr(C)]
pub struct Msg { pub xs: Vec<u64> }
#[no_mangle] pub extern "C" fn proc_mw_abi_version() -> i32 { 1 }
#[no_mangle] pub unsafe extern "C" fn mw_enter(req: *mut std::ffi::c_void, _resp: *mut std::ffi::c_void) -> i32 {
    let m = unsafe { &mut *(req as *mut Msg) };
    m.xs.sort();             // Vec::sort
    m.xs.reverse();
    0
}
"#;
    let so = proc_mw::compile::build_plugin_cached("types_vec", src, &std::env::temp_dir()).unwrap();
    let p = proc_mw::runtime::PluginOpaque::load(so.to_str().unwrap()).unwrap();
    #[repr(C)]
    struct Msg {
        xs: Vec<u64>,
    }
    let mut m = Msg { xs: vec![3, 1, 2] };
    let chain = OpaqueChain::new(vec![p.to_node()]);
    chain.exec(|_| {}, &mut m).unwrap();
    assert_eq!(m.xs, vec![3, 2, 1], "运行期编译插件对 Vec 字段调用 sort+reverse");
}

/// Box<i64> —— 运行期编译插件解引用修改堆上值
#[test]
fn plugin_with_box_deref_mut() {
    let src = r#"
#[repr(C)]
pub struct Msg { pub b: Box<i64> }
#[no_mangle] pub extern "C" fn proc_mw_abi_version() -> i32 { 1 }
#[no_mangle] pub unsafe extern "C" fn mw_enter(req: *mut std::ffi::c_void, _resp: *mut std::ffi::c_void) -> i32 {
    let m = unsafe { &mut *(req as *mut Msg) };
    *m.b += 5;               // Box deref_mut
    0
}
"#;
    let so = proc_mw::compile::build_plugin_cached("types_box", src, &std::env::temp_dir()).unwrap();
    let p = proc_mw::runtime::PluginOpaque::load(so.to_str().unwrap()).unwrap();
    #[repr(C)]
    struct Msg {
        b: Box<i64>,
    }
    let mut m = Msg { b: Box::new(1) };
    let chain = OpaqueChain::new(vec![p.to_node()]);
    chain.exec(|_| {}, &mut m).unwrap();
    assert_eq!(*m.b, 6, "运行期编译插件通过 Box deref 修改堆值");
}

/// Option<u32> —— 运行期编译插件模式匹配修字段
#[test]
fn plugin_with_option_field() {
    let src = r#"
#[repr(C)]
pub struct Msg { pub n: Option<u32> }
#[no_mangle] pub extern "C" fn proc_mw_abi_version() -> i32 { 1 }
#[no_mangle] pub unsafe extern "C" fn mw_enter(req: *mut std::ffi::c_void, _resp: *mut std::ffi::c_void) -> i32 {
    let m = unsafe { &mut *(req as *mut Msg) };
    if let Some(n) = m.n.as_mut() { *n += 1; }  // Option 字段
    0
}
"#;
    let so = proc_mw::compile::build_plugin_cached("types_option", src, &std::env::temp_dir()).unwrap();
    let p = proc_mw::runtime::PluginOpaque::load(so.to_str().unwrap()).unwrap();
    #[repr(C)]
    struct Msg {
        n: Option<u32>,
    }
    let mut m = Msg { n: Some(41) };
    let chain = OpaqueChain::new(vec![p.to_node()]);
    chain.exec(|_| {}, &mut m).unwrap();
    assert_eq!(m.n, Some(42), "运行期编译插件模式匹配修改 Option 字段");
}

// ===== 宿主侧正确性矩阵（全部类型种数）=====

#[test]
fn scalar_matrix() {
    // i32
    let c = OpaqueChain::new(vec![node(mi32_enter)]);
    let mut m = Mi32 { v: 1 };
    c.exec(|_| {}, &mut m).unwrap();
    assert_eq!(m.v, 2);
    // u64
    let c = OpaqueChain::new(vec![node(mu64_enter)]);
    let mut m = Mu64 { v: 2 };
    c.exec(|_| {}, &mut m).unwrap();
    assert_eq!(m.v, 7); // 2*3+1
    // f64
    let c = OpaqueChain::new(vec![node(mf64_enter)]);
    let mut m = Mf64 { v: 2.5 };
    c.exec(|_| {}, &mut m).unwrap();
    assert_eq!(m.v, 5.0);
    // bool
    let c = OpaqueChain::new(vec![node(mbool_enter)]);
    let mut m = Mbool { v: true };
    c.exec(|_| {}, &mut m).unwrap();
    assert!(!m.v);
    // char
    let c = OpaqueChain::new(vec![node(mchar_enter)]);
    let mut m = Mchar { v: 'a' };
    c.exec(|_| {}, &mut m).unwrap();
    assert_eq!(m.v, 'A');
}

#[test]
fn array_tuple_enum_matrix() {
    let c = OpaqueChain::new(vec![node(marr_enter)]);
    let mut m = MArr { v: [0; 16] };
    c.exec(|_| {}, &mut m).unwrap();
    assert_eq!(m.v[0], 1);

    let c = OpaqueChain::new(vec![node(mtup_enter)]);
    let mut m = MTup { v: (1, 2) };
    c.exec(|_| {}, &mut m).unwrap();
    assert_eq!(m.v, (2, 4));

    let c = OpaqueChain::new(vec![node(menum_enter)]);
    let mut m = MEnum { v: Kind::A };
    c.exec(|_| {}, &mut m).unwrap();
    assert_eq!(m.v, Kind::B);
}

#[test]
fn struct_matrix() {
    let c = OpaqueChain::new(vec![node(mplain_enter)]);
    let mut m = MPlain { v: Plain { a: 1, b: 2 } };
    c.exec(|_| {}, &mut m).unwrap();
    assert_eq!(m.v.a, 3);

    let c = OpaqueChain::new(vec![node(mnested_enter)]);
    let mut m = MNested { v: Nested { a: 1, b: Plain { a: 9, b: 0 } } };
    c.exec(|_| {}, &mut m).unwrap();
    assert_eq!(m.v.b.a, 10);

    // 带填充结构：布局守卫（size 16 / offset b==8）在编译期已断言
    let c = OpaqueChain::new(vec![node(mpadded_enter)]);
    let mut m = MPadded { v: Padded { a: 1, b: 100 } };
    c.exec(|_| {}, &mut m).unwrap();
    assert_eq!(m.v.b, 101, "padding 后字段偏移正确访问");
}

// ===== 明显不行的类型：证据化记录（D2 严谨性：不可行也要研究）=====

/// `dyn Trait` 胖指针 / `Box<dyn Trait>` / 闭包 —— 跨 .so 不可行。
///
/// 原因（证据）：`&dyn Trait` 是 (data, vtable) 胖指针，vtable 属于**定义它的那个
/// 编译单元**。宿主把自身 vtable 传进插件，插件源码里 `&dyn Trait` 的方法调用展开为
/// 对 vtable 槽位偏移的函数指针调用，偏移约定一致，但插件**无法按类型下传**——
/// 插件若用 `dyn Foo` 类型接收，其期望的 vtable 布局（含 `Foo` 的所有方法槽位）必须
/// 与宿主 vtable 完全一致，而 trait 定义在哪个 crate、方法顺序如何，跨编译单元无保证。
/// 这不是布局问题，是**类型身份跨动态链接边界不成立**（evcxr 同样不做）。
///
/// 结论：通过 c_void 共享类型定义，**不能**承载 `dyn Trait`/闭包；`Box<dyn>` 同。可行域
/// 是"布局稳定的具体类型"（本文件全矩阵）。这是 L7 极限的明确边界，不是缺陷。
#[test]
fn dyn_trait_boundary_documented() {
    // 编译期哨兵：本测试无运行断言，存在即记录"不可行域"（见上方 doc 证据）。
    // 若未来引入 trait-object 跨 .so，此测试应替换为实现，先列在 limits.md。
    assert!(true);
}
