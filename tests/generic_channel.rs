//! 泛型通道测试：Ctx<R, O> 承载任意 Rust 类型（不再是 i32 单型）
//! 用 HTTP 风格请求（String/Vec<u8>/u16）证明通道类型覆盖。

use proc_mw::dispatch::{Flow, MwError};
use proc_mw::generic::{self, Ctx};

#[derive(Debug, Clone, PartialEq)]
struct HttpReq {
    path: String,
    body: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq)]
struct HttpResp {
    status: u16,
    body: Vec<u8>,
}

// 守卫中间件：路径校验（操作 String 类型的方法）
fn guard(ctx: &mut Ctx<HttpReq, HttpResp>) -> Result<Flow, MwError> {
    if ctx.input.path.starts_with("/admin") {
        Err(MwError::Rejected("forbidden"))
    } else {
        Ok(Flow::Continue)
    }
}

// 变换中间件：往请求 body 追加字节（操作 Vec<u8> 的方法）
fn append_body(ctx: &mut Ctx<HttpReq, HttpResp>) -> Result<Flow, MwError> {
    ctx.input.body.push(b'!');
    Ok(Flow::Continue)
}

// 核心：生成响应（操作 u16/String/Vec<u8>）
fn core(ctx: &mut Ctx<HttpReq, HttpResp>) -> Result<HttpResp, MwError> {
    let path_len = ctx.input.path.len() as u16; // String::len
    let mut body = ctx.input.body.clone();
    body.extend_from_slice(b"-ok"); // Vec::extend_from_slice
    Ok(HttpResp {
        status: 200 + path_len,
        body,
    })
}

#[test]
fn generic_channel_carries_struct_request() {
    let req = HttpReq {
        path: "/api/users".to_string(),
        body: b"hello".to_vec(),
    };
    let chain = [guard as generic::FnMw<HttpReq, HttpResp>, append_body];
    let resp = generic::exec(&chain, core, req).unwrap();
    assert_eq!(resp.status, 200 + "/api/users".len() as u16);
    assert_eq!(resp.body, b"hello!-ok".to_vec());
    println!(
        "泛型通道：struct 请求走通 → status={} body={:?}",
        resp.status,
        String::from_utf8_lossy(&resp.body)
    );
}

#[test]
fn generic_channel_guard_short_circuits() {
    let req = HttpReq {
        path: "/private/forbidden".to_string(),
        body: vec![],
    };
    let chain = [guard as generic::FnMw<HttpReq, HttpResp>, append_body];
    let r = generic::exec(&chain, core, req);
    assert_eq!(r, Err(MwError::Rejected("forbidden")));
}

#[test]
fn generic_channel_works_for_primitives_too() {
    // 泛型通道同样能承载原始类型（与旧 i32 通道并存）
    let chain = [] as [generic::FnMw<i32, i32>; 0];
    let r = generic::exec(&chain, |ctx: &mut Ctx<i32, i32>| Ok(ctx.input + 1), 5).unwrap();
    assert_eq!(r, 6);
}

#[test]
fn generic_channel_core_panic_caught() {
    // sync 泛型通道的核心 panic 恢复（与 async_generic 对齐）
    let chain = [] as [generic::FnMw<HttpReq, HttpResp>; 0];
    let req = HttpReq {
        path: "/api".to_string(),
        body: vec![],
    };
    let r = generic::exec_catch(&chain, |_| -> Result<HttpResp, MwError> { panic!("bug") }, req);
    assert_eq!(r, Err(MwError::Rejected("core panicked")));
}
