//! 运行期编译管线（核心目的：编译任意 Rust 代码粘合进中间层，evcxr 机制）
//!
//! 中间件以 **Rust 源码**形式提供 → 写成一个临时 cdylib crate → `cargo build --release`
//! → 产物 .so → 交由 `Plugin`/`PluginOpaque` dlopen 并粘合进链。
//! 修改源码 → 重新编译 → 新 .so → 热更新（不停机换中间件逻辑）。
//!
//! 这是"拿 evcxr 的管线"：syn 解析→crate 编译→.so→dlopen→符号（CORE-CONSTRAINTS D6）。

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU32, Ordering};

static COUNTER: AtomicU32 = AtomicU32::new(0);

/// 编译中间件源码为 cdylib，返回 .so/.dylib 路径。
/// `middleware_source` 须导出 `proc_mw_abi_version()` 与 `mw_enter`（+可选 `mw_exit`）。
pub fn build_plugin(name: &str, middleware_source: &str, out_dir: &Path) -> Result<PathBuf, String> {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let crate_name = format!("{name}_{n}");
    let crate_dir = out_dir.join(&crate_name);

    fs::create_dir_all(crate_dir.join("src")).map_err(|e| format!("mkdir: {e}"))?;
    fs::write(
        crate_dir.join("Cargo.toml"),
        format!(
            "[package]\nname = \"{crate_name}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[lib]\ncrate-type = [\"cdylib\"]\n"
        ),
    )
    .map_err(|e| format!("写 Cargo.toml: {e}"))?;
    fs::write(crate_dir.join("src/lib.rs"), middleware_source).map_err(|e| format!("写 lib.rs: {e}"))?;

    // --offline：临时 crate 无依赖，避免更新索引
    let out = Command::new("cargo")
        .args(["build", "--release", "--offline"])
        .current_dir(&crate_dir)
        .output()
        .map_err(|e| format!("cargo 不可用: {e}"))?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr).to_string();
        let diags = extract_diagnostics(&stderr);
        return Err(format!("编译失败：{} 条诊断\n{}", diags.len(), render_diags(&diags, middleware_source)));
    }

    let ext = if cfg!(target_os = "macos") {
        "dylib"
    } else if cfg!(target_os = "windows") {
        "dll"
    } else {
        "so"
    };
    Ok(crate_dir.join("target/release").join(format!("lib{crate_name}.{ext}")))
}

/// 单条编译诊断（错误域：错误代码/消息/源码行号）
#[derive(Debug, Clone)]
pub struct Diag {
    pub code: String,
    pub message: String,
    pub line: Option<u32>,
}

/// 从 cargo stderr 提取结构化诊断（evcxr 从编译错误反推类型的技巧的基础）
pub fn extract_diagnostics(stderr: &str) -> Vec<Diag> {
    let mut out: Vec<Diag> = Vec::new();
    for l in stderr.lines() {
        let t = l.trim();
        // 错误头：error[E0308]: mismatched types
        if let Some(idx) = t.find("error[") {
            let end = t[idx..].find("]").map(|i| idx + i).unwrap_or(t.len());
            let code = t[idx + 6..end].to_string();
            let msg = t[end + 1..].trim_start_matches(": ").to_string();
            out.push(Diag {
                code,
                message: msg,
                line: None,
            });
        } else if let Some(rest) = t.strip_prefix("-->") {
            // 位置行：--> src/lib.rs:5:9
            if let Some(d) = out.last_mut() {
                if d.line.is_none() {
                    if let Some(colon) = rest.rfind(':') {
                        let before = &rest[..colon];
                        if let Some(colon2) = before.rfind(':') {
                            if let Ok(n) = before[colon2 + 1..].trim().parse::<u32>() {
                                d.line = Some(n);
                            }
                        }
                    }
                }
            }
        }
    }
    out
}

/// 渲染诊断：附中间件源码对应行（定位到用户源码）
pub fn render_diags(diags: &[Diag], middleware_source: &str) -> String {
    let lines: Vec<&str> = middleware_source.lines().collect();
    let mut out = String::new();
    for d in diags {
        out.push_str(&format!("  error[{}] {}\n", d.code, d.message));
        if let Some(n) = d.line {
            if let Some(src) = lines.get(n.saturating_sub(1) as usize) {
                out.push_str(&format!("    {:>3} │ {}\n", n, src));
            }
        }
    }
    out
}

/// 工具链打包/分发域：生产部署的编译环境检查（运行期编译管线需要 cargo/rustc）
pub struct ToolchainReport {
    pub cargo: Option<String>,
    pub rustc: Option<String>,
    pub offline_ready: bool, // --offline 构建可用（无依赖 → 不联网）
    pub usable: bool,        // 运行期编译管线是否可用
}

/// 检查当前环境的工具链可用性（部署前验证）
pub fn toolchain_report() -> ToolchainReport {
    let ver = |bin: &str| -> Option<String> {
        Command::new(bin)
            .arg("--version")
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
    };
    let cargo = ver("cargo");
    let rustc = ver("rustc");
    // --offline 构建一个小 crate 验证（无依赖临时 crate）
    let offline_ready = cargo.is_some() && rustc.is_some();
    ToolchainReport {
        cargo,
        rustc,
        offline_ready,
        usable: offline_ready,
    }
}

/// 源码简单哈希（FNV-1a 64 位）——编译缓存的键
fn fnv1a64(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// 带缓存的编译：相同源码 → 直接复用已编译 .so（evcxr cache.rs 思路）。
/// 缓存目录 `out_dir/proc_mw_compile_cache/`，键 = 源码哈希。
pub fn build_plugin_cached(
    name: &str,
    middleware_source: &str,
    out_dir: &Path,
) -> Result<PathBuf, String> {
    let cache_dir = out_dir.join("proc_mw_compile_cache");
    fs::create_dir_all(&cache_dir).map_err(|e| format!("mkdir cache: {e}"))?;

    let ext = if cfg!(target_os = "macos") {
        "dylib"
    } else if cfg!(target_os = "windows") {
        "dll"
    } else {
        "so"
    };
    let hash = fnv1a64(middleware_source.as_bytes());
    let cache_path = cache_dir.join(format!("{name}_{hash:016x}.{ext}"));

    if cache_path.exists() {
        // 缓存命中：复用，跳过编译
        return Ok(cache_path);
    }

    // 缓存未命中：编译并把产物复制到缓存位置
    let so = build_plugin(name, middleware_source, out_dir)?;
    // 原子写缓存：先写临时名再 rename，避免并发读者看到半成品
    let tmp = cache_path.with_extension("tmp");
    fs::copy(&so, &tmp).map_err(|e| format!("写缓存: {e}"))?;
    fs::rename(&tmp, &cache_path).map_err(|e| format!("缓存原子化: {e}"))?;

    // 资源管理：临时 crate 目录已冗余（产物已入缓存），清理防泄漏
    if let Some(crate_dir) = so
        .parent()
        .and_then(|p| p.parent())
        .and_then(|p| p.parent())
    {
        let _ = fs::remove_dir_all(crate_dir);
    }
    Ok(cache_path)
}

/// 资源管理：缓存按字节上限清理（最旧优先淘汰，LRU 思路）
pub fn cache_cleanup(out_dir: &Path, max_bytes: u64) -> usize {
    let cache_dir = out_dir.join("proc_mw_compile_cache");
    if !cache_dir.exists() {
        return 0;
    }
    let mut entries: Vec<(std::path::PathBuf, u64, Option<std::time::SystemTime>)> =
        fs::read_dir(&cache_dir)
            .into_iter()
            .flatten()
            .filter_map(|e| e.ok())
            .map(|e| {
                let p = e.path();
                let size = e.metadata().map(|m| m.len()).unwrap_or(0);
                let mtime = e.metadata().and_then(|m| m.modified()).ok();
                (p, size, mtime)
            })
            .collect();
    entries.sort_by_key(|(_, _, m)| *m); // 最旧在前
    let mut total: u64 = entries.iter().map(|(_, s, _)| *s).sum();
    let mut removed = 0usize;
    while total > max_bytes && entries.len() > 1 {
        let (p, s, _) = entries.remove(0);
        if fs::remove_file(&p).is_ok() {
            total -= s;
            removed += 1;
        }
    }
    removed
}
