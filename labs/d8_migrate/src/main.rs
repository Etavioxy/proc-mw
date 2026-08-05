//! D8 迁移工具：识别 handler → 包装进中间件链（syn AST 改写，可回滚）
//!
//! 管线：静态分析（识别 `fn handle_*` 候选核心）→ AST 改写（包装进 chain_exec）
//! → 渐进/回滚（.bak 快照）→ 编译反馈（输出为合法 Rust，依赖 proc-mw）。
//!
//! 用法：
//!   cargo run -p d8_migrate -- samples/legacy.rs            # dry-run 打印变换
//!   cargo run -p d8_migrate -- samples/legacy.rs --apply    # 写 .mw.rs + .bak 快照
//!
//! 行为等价验证：tests/migrated_behavior.rs include 迁移产物并断言输出一致。

use std::env;
use std::fs;

use proc_macro2::TokenStream;
use quote::quote;
use syn::visit_mut::VisitMut;
use syn::{Expr, ItemFn};

/// 把 handler 体内对参数 `input` 的引用重命名为 `ctx.input`（进入核心闭包后）
struct RenameInput;
impl VisitMut for RenameInput {
    fn visit_expr_mut(&mut self, e: &mut Expr) {
        if let Expr::Path(ep) = e {
            if ep.qself.is_none() && ep.path.is_ident("input") {
                *e = syn::parse_quote!(ctx.input);
            }
        }
        syn::visit_mut::visit_expr_mut(self, e);
    }
}

/// 候选核心：`fn handle_*(input: i32) -> i32`
fn is_handler(f: &ItemFn) -> bool {
    let name = f.sig.ident.to_string();
    if !name.starts_with("handle_") {
        return false;
    }
    if f.sig.inputs.len() != 1 {
        return false;
    }
    match &f.sig.output {
        syn::ReturnType::Type(_, ty) => quote!(#ty).to_string().contains("i32"),
        _ => false,
    }
}

/// 包装：原函数体变为核心闭包，外层经 chain_exec（本 MVP 空链=恒等，保行为等价）
fn wrap_handler(f: &mut ItemFn) {
    RenameInput.visit_block_mut(&mut f.block);
    let body = &f.block;
    f.block = Box::new(syn::parse_quote!({
        proc_mw::chain_exec(
            &[], // MVP 空链：包装=恒等；后续把内联横切逻辑抽成中间件填入此处
            |ctx: &mut proc_mw::dispatch::Ctx| -> Result<i32, proc_mw::dispatch::MwError> #body,
            input,
        )
        .unwrap_or_default()
    }));
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let file = args.get(1).unwrap_or_else(|| {
        eprintln!("usage: d8_migrate <file.rs> [--apply]");
        std::process::exit(1);
    });
    let apply = args.contains(&"--apply".to_string());

    let src = fs::read_to_string(file).expect("读取源文件失败");
    let mut ast: syn::File = syn::parse_str(&src).expect("解析失败");

    let mut count = 0usize;
    let mut wrapped: Vec<String> = Vec::new();
    for item in &mut ast.items {
        if let syn::Item::Fn(f) = item {
            if is_handler(f) {
                wrapped.push(f.sig.ident.to_string());
                wrap_handler(f);
                count += 1;
            }
        }
    }

    let out = quote!(#ast).to_string();
    println!("// D8 迁移：识别 {} 个候选核心，包装 {} 个", wrapped.len(), count);
    for w in &wrapped {
        println!("//   包装: {w}");
    }

    if apply {
        let out_path = format!("{file}.mw.rs");
        let bak_path = format!("{file}.bak");
        fs::write(&out_path, format!("{out}\n")).expect("写产物失败");
        fs::write(&bak_path, &src).expect("写快照失败");
        println!("// 已写 {out_path}（回滚快照 {bak_path}，按域渐进可回滚）");
    } else {
        println!("{out}");
    }
}
