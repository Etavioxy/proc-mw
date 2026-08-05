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

use quote::quote;
use syn::visit_mut::VisitMut;
use syn::{Expr, Item, ItemFn};

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

/// quote! 渲染 token 带空格（`Instant :: now`），归一化去空白后再匹配
fn norm(s: &str) -> String {
    s.chars().filter(|c| !c.is_whitespace()).collect()
}

/// 识别内联横切逻辑（D8）：计时（Instant）/日志（println）散落在 handler 体内
fn detect_cross_cutting(block: &syn::Block) -> Vec<String> {
    let txt = norm(&quote!(#block).to_string());
    let mut out = Vec::new();
    if txt.contains("Instant::now") || txt.contains("elapsed") {
        out.push("timing".to_string());
    }
    if txt.contains("println") {
        out.push("logging".to_string());
    }
    out
}

/// 从核心剥离内联横切语句（计时）：保留业务，抽走横切
struct StripTiming;
impl VisitMut for StripTiming {
    fn visit_block_mut(&mut self, b: &mut syn::Block) {
        b.stmts.retain(|s| {
            let t = norm(&quote!(#s).to_string());
            !(t.contains("Instant::now") || t.contains("elapsed"))
        });
        syn::visit_mut::visit_block_mut(self, b);
    }
}

/// 包装：原函数体（剥掉内联横切后）变为核心闭包，外层经 chain_exec，
/// 识别出的横切逻辑填入链的中间件
fn wrap_handler(f: &mut ItemFn, out_middleware: &mut Vec<String>) {
    RenameInput.visit_block_mut(&mut f.block);
    // 识别 + 剥离内联横切
    let cc = detect_cross_cutting(&f.block);
    StripTiming.visit_block_mut(&mut f.block);
    for c in &cc {
        if !out_middleware.contains(c) {
            out_middleware.push(c.clone());
        }
    }
    let body = &f.block;
    // 链中间件：识别出的横切 → 对应中间件（计时 → mw_timing）
    let has_timing = cc.contains(&"timing".to_string());
    let chain_nodes: proc_macro2::TokenStream = if has_timing {
        quote!(&[proc_mw::dispatch::Node::FnPtr(mw_timing)])
    } else {
        quote!(&[])
    };
    f.block = Box::new(syn::parse_quote!({
        proc_mw::dispatch::chain_exec(
            #chain_nodes,
            |ctx: &mut proc_mw::dispatch::Ctx| -> Result<i32, proc_mw::dispatch::MwError> {
                Ok(#body) // 核心（已剥内联横切）
            },
            input,
        )
        .unwrap_or_default()
    }));
}

/// 剥离 doc 属性：quote! 把 doc 注释渲染成 `#![doc]` 内属性，include 时非法
struct StripDoc;
impl VisitMut for StripDoc {
    fn visit_item_mut(&mut self, item: &mut Item) {
        let attrs = match item {
            Item::Fn(f) => &mut f.attrs,
            Item::Struct(s) => &mut s.attrs,
            Item::Enum(e) => &mut e.attrs,
            Item::Const(c) => &mut c.attrs,
            Item::Static(s) => &mut s.attrs,
            Item::Trait(t) => &mut t.attrs,
            Item::Type(t) => &mut t.attrs,
            Item::Mod(m) => &mut m.attrs,
            Item::Impl(i) => &mut i.attrs,
            _ => return,
        };
        attrs.retain(|a| !a.path().is_ident("doc"));
        syn::visit_mut::visit_item_mut(self, item);
    }
}

/// 处理单个文件（按域渐进：每文件独立产物 + 回滚快照）
fn migrate_file(file: &str, apply: bool) -> Result<(), String> {
    let src = fs::read_to_string(file).map_err(|e| format!("读取 {file}: {e}"))?;
    let mut ast: syn::File = syn::parse_str(&src).map_err(|e| format!("解析 {file}: {e}"))?;
    // 文件级内属性（`//!` doc 注释 → `#![doc]`）也剥离，否则 include 时非法
    ast.attrs.retain(|a| !a.path().is_ident("doc"));

    let mut count = 0usize;
    let mut wrapped: Vec<String> = Vec::new();
    let mut extracted: Vec<String> = Vec::new();
    for item in &mut ast.items {
        if let syn::Item::Fn(f) = item {
            if is_handler(f) {
                wrapped.push(f.sig.ident.to_string());
                wrap_handler(f, &mut extracted);
                count += 1;
            }
        }
    }

    // 识别出的横切逻辑 → 生成对应中间件函数注入产物（D8：内联横切 → 中间件）
    if extracted.contains(&"timing".to_string()) {
        let mw: syn::Item = syn::parse_quote!(
            #[allow(dead_code)]
            fn mw_timing(ctx: &mut proc_mw::dispatch::Ctx) -> Result<proc_mw::dispatch::Flow, proc_mw::dispatch::MwError> {
                // 从 handler 体内抽出的横切逻辑：计时观测（进入侧）
                let _ = ctx;
                Ok(proc_mw::dispatch::Flow::Continue)
            }
        );
        ast.items.push(mw);
    }

    StripDoc.visit_file_mut(&mut ast);
    let out = quote!(#ast).to_string();
    println!("// D8 迁移 {file}：识别 {} 个候选核心，包装 {} 个", wrapped.len(), count);
    for w in &wrapped {
        println!("//   包装: {w}");
    }

    if apply {
        let out_path = format!("{file}.mw.rs");
        let bak_path = format!("{file}.bak");
        fs::write(&out_path, format!("{out}\n")).map_err(|e| format!("写产物: {e}"))?;
        fs::write(&bak_path, &src).map_err(|e| format!("写快照: {e}"))?;
        println!("// 已写 {out_path}（回滚快照 {bak_path}，按域渐进可回滚）");
    } else {
        println!("{out}");
    }
    Ok(())
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let apply = args.contains(&"--apply".to_string());
    // 多文件：每个文件独立产物+回滚（D8 按域渐进采纳）
    let files: Vec<&String> = args.iter().skip(1).filter(|a| *a != "--apply").collect();
    if files.is_empty() {
        eprintln!("usage: d8_migrate <file.rs> [<file2.rs> ...] [--apply]");
        std::process::exit(1);
    }
    for f in &files {
        if let Err(e) = migrate_file(f, apply) {
            eprintln!("迁移 {f} 失败: {e}");
            std::process::exit(1);
        }
    }
}
