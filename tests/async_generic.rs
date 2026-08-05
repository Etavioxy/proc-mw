//! async 泛型通道测试：异步 + 任意类型（struct 请求 + 真实 await + 短路）

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use proc_mw::async_generic::{AsyncChain, AsyncMw};
use proc_mw::async_mw::YieldOnce;
use proc_mw::dispatch::{Flow, MwError};
use proc_mw::generic::Ctx;

#[derive(Debug, PartialEq)]
struct HttpReq {
    path: String,
    body: Vec<u8>,
}

#[derive(Debug, PartialEq)]
struct HttpResp {
    status: u16,
    body: Vec<u8>,
}

/// async 鉴权中间件：真实 await + 操作 String 字段 + 短路
struct AsyncAuth {
    required: &'static str,
}
impl AsyncMw<HttpReq, HttpResp> for AsyncAuth {
    fn call<'a>(
        &'a self,
        ctx: &'a mut Ctx<HttpReq, HttpResp>,
    ) -> Pin<Box<dyn Future<Output = Result<Flow, MwError>> + Send + 'a>> {
        Box::pin(async move {
            YieldOnce::new().await; // 真实挂起/恢复
            if ctx.input.path.starts_with(self.required) {
                Err(MwError::Rejected("unauthorized"))
            } else {
                Ok(Flow::Continue)
            }
        })
    }
}

fn core(ctx: &mut Ctx<HttpReq, HttpResp>) -> Result<HttpResp, MwError> {
    let mut body = ctx.input.body.clone();
    body.extend_from_slice(b"-ok");
    Ok(HttpResp {
        status: 200,
        body,
    })
}

#[test]
fn async_generic_chain_works() {
    let chain = AsyncChain::new(vec![Arc::new(AsyncAuth { required: "/admin" })]);
    let req = HttpReq {
        path: "/api/users".to_string(),
        body: b"hi".to_vec(),
    };
    let r = futures::executor::block_on(chain.exec(core, req)).unwrap();
    assert_eq!(r.status, 200);
    assert_eq!(r.body, b"hi-ok".to_vec());
}

#[test]
fn async_generic_short_circuit() {
    let chain = AsyncChain::new(vec![Arc::new(AsyncAuth { required: "/admin" })]);
    let req = HttpReq {
        path: "/admin/x".to_string(),
        body: vec![],
    };
    let r = futures::executor::block_on(chain.exec(core, req));
    assert_eq!(r, Err(MwError::Rejected("unauthorized")));
}

#[test]
fn async_generic_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<AsyncChain<HttpReq, HttpResp>>();
}

#[test]
fn async_generic_core_panic_caught() {
    let chain = AsyncChain::new(vec![Arc::new(AsyncAuth { required: "/admin" })]);
    let req = HttpReq {
        path: "/api".to_string(),
        body: vec![],
    };
    let r = futures::executor::block_on(chain.exec_catch(|_| panic!("generic bug"), req));
    assert_eq!(r, Err(MwError::Rejected("core panicked")));
}
