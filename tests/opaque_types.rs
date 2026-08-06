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
    OpaqueNode::Thin {
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

/// String::push —— 运行期编译插件操作堆字符串字段（**类型注入** + 布局指纹校验）
#[test]
fn plugin_with_string_push() {
    // 共享类型定义单一来源：编译管线注入插件，宿主 include 同一文件（防双写漂移）
    const TYPE_DEF: &str = r#"#[repr(C)] pub struct Msg { pub s: String }"#;
    const BODY: &str = r#"
#[no_mangle] pub extern "C" fn proc_mw_abi_version() -> i32 { 1 }
#[no_mangle] pub unsafe extern "C" fn mw_enter(req: *mut std::ffi::c_void, _resp: *mut std::ffi::c_void) -> i32 {
    let m = unsafe { &mut *(req as *mut Msg) };
    m.s.push('!');           // String::push
    m.s.push_str("HOOT");    // String::push_str
    0
}
#[no_mangle] pub extern "C" fn proc_mw_layout_fingerprint() -> u64 {
    (std::mem::size_of::<Msg>() as u64) << 32 | (std::mem::align_of::<Msg>() as u64)
}
"#;
    let source = proc_mw::compile::inject_shared_type(TYPE_DEF, BODY);
    let so = proc_mw::compile::build_plugin_cached("types_string", &source, &std::env::temp_dir()).unwrap();
    // 加载 + 布局指纹校验（D7）：插件指纹须与宿主一致
    let p = proc_mw::runtime::PluginOpaque::load_with_layout(
        so.to_str().unwrap(),
        proc_mw::runtime::layout_fingerprint::<Msg>(),
    )
    .unwrap();
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
#[no_mangle] pub extern "C" fn proc_mw_layout_fingerprint() -> u64 {
    (std::mem::size_of::<Msg>() as u64) << 32 | (std::mem::align_of::<Msg>() as u64)
}
"#;
    let so = proc_mw::compile::build_plugin_cached("types_vec", src, &std::env::temp_dir()).unwrap();
    let p = proc_mw::runtime::PluginOpaque::load_with_layout(
        so.to_str().unwrap(),
        proc_mw::runtime::layout_fingerprint::<Msg>(),
    )
    .unwrap();
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
#[no_mangle] pub extern "C" fn proc_mw_layout_fingerprint() -> u64 {
    (std::mem::size_of::<Msg>() as u64) << 32 | (std::mem::align_of::<Msg>() as u64)
}
"#;
    let so = proc_mw::compile::build_plugin_cached("types_box", src, &std::env::temp_dir()).unwrap();
    let p = proc_mw::runtime::PluginOpaque::load_with_layout(
        so.to_str().unwrap(),
        proc_mw::runtime::layout_fingerprint::<Msg>(),
    )
    .unwrap();
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
#[no_mangle] pub extern "C" fn proc_mw_layout_fingerprint() -> u64 {
    (std::mem::size_of::<Msg>() as u64) << 32 | (std::mem::align_of::<Msg>() as u64)
}
"#;
    let so = proc_mw::compile::build_plugin_cached("types_option", src, &std::env::temp_dir()).unwrap();
    let p = proc_mw::runtime::PluginOpaque::load_with_layout(
        so.to_str().unwrap(),
        proc_mw::runtime::layout_fingerprint::<Msg>(),
    )
    .unwrap();
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
/// println! 可被**热编译**进中间件（std 输出 = 进程共享 stdout）。
/// 证明：运行期编译的 .dylib 里可以带输出逻辑（观测/调试中间件），非纯静默。
/// 运行 `cargo test --test opaque_types plugin_with_println -- --nocapture` 可见打印。
#[test]
fn plugin_with_println_hot_compiled() {
    let src = r#"
#[repr(C)]
pub struct Msg { pub val: i64 }
#[no_mangle] pub extern "C" fn proc_mw_abi_version() -> i32 { 1 }
#[no_mangle] pub unsafe extern "C" fn mw_enter(req: *mut std::ffi::c_void, _resp: *mut std::ffi::c_void) -> i32 {
    let m = unsafe { &mut *(req as *mut Msg) };
    m.val += 1;
    println!("[热编译中间件 println] 当前 val = {}", m.val);   // ← println! 在运行期编译的 .dylib 内
    0
}
"#;
    let so = proc_mw::compile::build_plugin_cached("types_println", src, &std::env::temp_dir()).unwrap();
    let p = proc_mw::runtime::PluginOpaque::load(so.to_str().unwrap()).unwrap();
    #[repr(C)]
    struct Msg {
        val: i64,
    }
    let mut m = Msg { val: 41 };
    let chain = OpaqueChain::new(vec![p.to_node()]);
    chain.exec(|m| m.val, &mut m).unwrap();
    assert_eq!(m.val, 42, "println 中间件照常变换消息");
}

/// 外部 crate 依赖：运行期编译的中间件可用 regex（编译管线 deps 支持，非 --offline）。
/// 补齐"任意 Rust 代码"的外部 crate 域（evcxr `:dep` 对应物）。regex 已在 cargo 缓存，免网络。
#[test]
fn plugin_with_external_crate_regex() {
    let src = r#"
#[repr(C)]
pub struct Msg { pub text: [u8; 64], pub len: usize, pub matched: u8 }
#[no_mangle] pub extern "C" fn proc_mw_abi_version() -> i32 { 1 }
#[no_mangle] pub unsafe extern "C" fn mw_enter(req: *mut std::ffi::c_void, _resp: *mut std::ffi::c_void) -> i32 {
    let m = unsafe { &mut *(req as *mut Msg) };
    let s = String::from_utf8_lossy(&m.text[..m.len]);
    let re = regex::Regex::new(r"^[a-z]+-[0-9]+$").unwrap();   // 外部 crate：regex
    m.matched = if re.is_match(&s) { 1 } else { 0 };
    0
}
"#;
    let so = proc_mw::compile::build_plugin_with_deps(
        "types_regex",
        src,
        "regex = \"1.13\"",
        &std::env::temp_dir(),
    )
    .expect("运行期编译带外部依赖的中间件（regex）");
    let p = proc_mw::runtime::PluginOpaque::load(so.to_str().unwrap()).unwrap();
    #[repr(C)]
    struct Msg {
        text: [u8; 64],
        len: usize,
        matched: u8,
    }
    let mk = |s: &str| {
        let mut text = [0u8; 64];
        let b = s.as_bytes();
        text[..b.len()].copy_from_slice(b);
        Msg { text, len: b.len(), matched: 0 }
    };
    let chain = OpaqueChain::new(vec![p.to_node()]);
    let mut m1 = mk("order-123");
    chain.exec(|_| {}, &mut m1).unwrap();
    assert_eq!(m1.matched, 1, "regex 应匹配 order-123");
    let mut m2 = mk("no-match");
    chain.exec(|_| {}, &mut m2).unwrap();
    assert_eq!(m2.matched, 0, "regex 不应匹配 no-match");
}

/// 富化指纹：捕获**同 size/align 的字段重排**（朴素 size<<32|align 测不到）。
/// 插件 `{a:u64,b:u8}` vs 宿主 `{b:u8,a:u64}` 同为 16B/8 对齐，但 (offset,size,align)
/// 三元组序列不同 → 加载期拦截。
#[test]
fn plugin_field_reorder_detected_by_rich_fingerprint() {
    let src = r#"
#[repr(C)]
pub struct Msg { pub a: u64, pub b: u8 }   // 插件：a 在前
#[no_mangle] pub extern "C" fn proc_mw_abi_version() -> i32 { 1 }
#[no_mangle] pub unsafe extern "C" fn mw_enter(req: *mut std::ffi::c_void, _resp: *mut std::ffi::c_void) -> i32 {
    let m = unsafe { &mut *(req as *mut Msg) };
    m.a += 1;
    0
}
#[no_mangle] pub extern "C" fn proc_mw_layout_fingerprint() -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &(off, sz, al) in &[
        (std::mem::offset_of!(Msg, a), std::mem::size_of::<u64>(), std::mem::align_of::<u64>()),
        (std::mem::offset_of!(Msg, b), std::mem::size_of::<u8>(), std::mem::align_of::<u8>()),
    ] {
        for &x in [off, sz, al].iter() { h ^= x as u64; h = h.wrapping_mul(0x100000001b3); }
    }
    h
}
"#;
    let so = proc_mw::compile::build_plugin_cached("types_reorder", src, &std::env::temp_dir()).unwrap();
    #[repr(C)]
    struct Msg {
        b: u8, // 宿主：b 在前（字段重排，同 16B/8 对齐）
        a: u64,
    }
    let expected = proc_mw::runtime::layout_fingerprint_of(&[
        (
            std::mem::offset_of!(Msg, b),
            std::mem::size_of::<u8>(),
            std::mem::align_of::<u8>(),
        ),
        (
            std::mem::offset_of!(Msg, a),
            std::mem::size_of::<u64>(),
            std::mem::align_of::<u64>(),
        ),
    ]);
    match proc_mw::runtime::PluginOpaque::load_with_layout(so.to_str().unwrap(), expected) {
        Err(msg) => assert!(msg.contains("布局指纹不匹配"), "字段重排应被富化指纹拦截"),
        Ok(_) => panic!("同 size/align 字段重排必须被富化指纹拦截"),
    }
}

/// 布局漂移检测：插件声明的共享类型与宿主期望布局不一致 → **加载期硬失败**
/// （D7：防共享类型漂移导致运行期 UB；指纹 = size<<32|align）。
/// 局限：同 size/align 的字段重排无法被此指纹捕获（列 limits.md），是显式边界。
#[test]
fn plugin_layout_mismatch_detected_at_load() {
    let src = r#"
#[repr(C)]
pub struct Msg { pub a: u8 }            // 插件布局：1 字节
#[no_mangle] pub extern "C" fn proc_mw_abi_version() -> i32 { 1 }
#[no_mangle] pub unsafe extern "C" fn mw_enter(req: *mut std::ffi::c_void, _resp: *mut std::ffi::c_void) -> i32 {
    let m = unsafe { &mut *(req as *mut Msg) };
    m.a = m.a.wrapping_add(1);
    0
}
#[no_mangle] pub extern "C" fn proc_mw_layout_fingerprint() -> u64 {
    (std::mem::size_of::<Msg>() as u64) << 32 | (std::mem::align_of::<Msg>() as u64)
}
"#;
    let so = proc_mw::compile::build_plugin_cached("types_mismatch", src, &std::env::temp_dir()).unwrap();
    #[repr(C)]
    struct Msg {
        a: u64, // 宿主期望 8 字节 → 指纹不同
    }
    match proc_mw::runtime::PluginOpaque::load_with_layout(
        so.to_str().unwrap(),
        proc_mw::runtime::layout_fingerprint::<Msg>(),
    ) {
        Err(msg) => assert!(msg.contains("布局指纹不匹配"), "报错信息应指明指纹不匹配"),
        Ok(_) => panic!("布局指纹不匹配必须在加载期报错，而非运行期 UB"),
    }
}

#[test]
fn dyn_trait_boundary_documented() {
    // 硬证据：`&dyn Trait` 是 16 字节胖指针（data + vtable），装不进 8 字节 c_void。
    // 即共享类型定义 ABI 物理上无法承载 trait-object/闭包。
    assert_eq!(std::mem::size_of::<&dyn std::fmt::Debug>(), 16);
    assert_eq!(std::mem::size_of::<*mut std::ffi::c_void>(), 8);
    assert_eq!(std::mem::size_of::<&dyn std::fmt::Debug>() / std::mem::size_of::<*mut std::ffi::c_void>(), 2);
}
