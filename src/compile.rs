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
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

static COUNTER: AtomicU32 = AtomicU32::new(0);
/// 全局构建锁：串行化插件 cargo 构建——共享 target-dir（依赖复用）安全，
/// 并发构建排队不冲突（不同插件可并行编译的收益 < 共享依赖的收益）。
static BUILD_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
/// 编译管线自身可观测性（D8 工具链域）：构建次数 / 缓存命中
static TOTAL_BUILDS: AtomicU64 = AtomicU64::new(0);
static CACHE_HITS: AtomicU64 = AtomicU64::new(0);

/// 编译中间件源码为 cdylib，返回 .so/.dylib 路径。
/// `middleware_source` 须导出 `proc_mw_abi_version()` 与 `mw_enter`（+可选 `mw_exit`）。
pub fn build_plugin(name: &str, middleware_source: &str, out_dir: &Path) -> Result<PathBuf, String> {
    build_plugin_with_deps(name, middleware_source, "", out_dir)
}

/// 编译中间件源码为 cdylib，支持**外部 crate 依赖**（`deps` = Cargo.toml `[dependencies]`
/// 段内容，空串 = 无依赖）。
///
/// 依赖策略：`deps` 为空 → `--offline`（无依赖不必联网更新索引）；`deps` 非空 → 走在线
/// （cargo 解析并利用共享注册表缓存；未缓存的会从 crates.io 拉取）。这补齐了
/// "任意 Rust 代码"的外部 crate 域（evcxr `:dep` 对应物）——中间件可用 serde/regex/rand 等。
pub fn build_plugin_with_deps(
    name: &str,
    middleware_source: &str,
    deps: &str,
    out_dir: &Path,
) -> Result<PathBuf, String> {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let crate_name = format!("{name}_{n}");
    let crate_dir = out_dir.join(&crate_name);

    fs::create_dir_all(crate_dir.join("src")).map_err(|e| format!("mkdir: {e}"))?;
    let cargo_toml = if deps.trim().is_empty() {
        format!(
            "[package]\nname = \"{crate_name}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[lib]\ncrate-type = [\"cdylib\"]\n"
        )
    } else {
        format!(
            "[package]\nname = \"{crate_name}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[lib]\ncrate-type = [\"cdylib\"]\n\n[dependencies]\n{deps}\n"
        )
    };
    fs::write(crate_dir.join("Cargo.toml"), cargo_toml).map_err(|e| format!("写 Cargo.toml: {e}"))?;
    fs::write(crate_dir.join("src/lib.rs"), middleware_source).map_err(|e| format!("写 lib.rs: {e}"))?;

    // 无依赖 → --offline（避免更新索引）；有依赖 → 在线（用 cargo 缓存/拉取）
    let mut cmd = Command::new("cargo");
    cmd.args(["build", "--release"]).current_dir(&crate_dir);
    // 共享 target-dir：不同插件构建复用已编译依赖（bevy_ecs 等重依赖只编一次），
    // 否则每个插件临时 crate 的 fresh target 会重编全部依赖（实测 16.3s → 秒级）
    let shared_target = out_dir.join("proc_mw_plugin_target");
    cmd.arg("--target-dir").arg(&shared_target);
    if deps.trim().is_empty() {
        cmd.arg("--offline");
    }
    let _guard = BUILD_LOCK.lock().unwrap(); // 串行化：共享 target-dir 并发安全
    let out = match cmd.output() {
        Ok(o) => o,
        Err(e) => {
            let _ = fs::remove_dir_all(&crate_dir); // 失败也清理临时目录（防泄漏）
            return Err(format!("cargo 不可用: {e}"));
        }
    };
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr).to_string();
        let diags = extract_diagnostics(&stderr);
        let _ = fs::remove_dir_all(&crate_dir); // 编译失败清理临时目录（防泄漏）
        return Err(format!("编译失败：{} 条诊断\n{}", diags.len(), render_diags(&diags, middleware_source)));
    }
    let _ = fs::remove_dir_all(&crate_dir); // 成功：临时 crate 冗余（.so 在共享 target），清理

    let ext = if cfg!(target_os = "macos") {
        "dylib"
    } else if cfg!(target_os = "windows") {
        "dll"
    } else {
        "so"
    };
    // 产物在共享 target-dir（--target-dir），非 crate_dir/target/
    Ok(shared_target.join("release").join(format!("lib{crate_name}.{ext}")))
}

/// 把**共享类型定义**注入插件源码（D6 工具链域，消除宿主/插件双写漂移）。
///
/// 宿主与运行期编译插件各自需要同一 `#[repr(C)]` 类型。若两边各写一遍，改类型时
/// 极易漂移。此函数把类型定义作为**单一来源**注入插件 crate 顶部，插件源码只写
/// 业务变换（`mw_enter`）。宿主侧可用 `include!(type_def)` 引入同一文件定义 struct。
pub fn inject_shared_type(shared_type_def: &str, middleware_body: &str) -> String {
    let mut source = String::new();
    source.push_str("// 共享类型定义（编译管线注入；宿主经 include! 引入同一文件，同源）\n");
    source.push_str(shared_type_def);
    source.push('\n');
    source.push_str("// 中间件本体（运行期编译的任意 Rust）\n");
    source.push_str(middleware_body);
    source.push('\n');
    source
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

/// 编译管线统计（工具链自身可观测性）：总请求 / 缓存命中 / 命中率
pub fn pipeline_stats() -> (u64, u64) {
    (
        TOTAL_BUILDS.load(Ordering::SeqCst),
        CACHE_HITS.load(Ordering::SeqCst),
    )
}

/// 工具链指纹（懒求值）：rustc 版本哈希——缓存按工具链失效（升级后旧 .so 不用）
static TOOLCHAIN_FP: std::sync::OnceLock<u64> = std::sync::OnceLock::new();
fn toolchain_fingerprint() -> u64 {
    *TOOLCHAIN_FP.get_or_init(|| {
        let ver = toolchain_report().rustc.unwrap_or_default();
        fnv1a64(ver.as_bytes())
    })
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
    // 缓存键 = 源码哈希 ^ 工具链指纹（升级 rustc 后旧 .so 不再命中）
    let hash = fnv1a64(middleware_source.as_bytes()) ^ toolchain_fingerprint().rotate_left(32);
    let cache_path = cache_dir.join(format!("{name}_{hash:016x}.{ext}"));

    if cache_path.exists() {
        // 缓存命中：复用，跳过编译
        CACHE_HITS.fetch_add(1, Ordering::SeqCst);
        TOTAL_BUILDS.fetch_add(1, Ordering::SeqCst);
        return Ok(cache_path);
    }
    TOTAL_BUILDS.fetch_add(1, Ordering::SeqCst);

    // 缓存未命中：编译并把产物复制到缓存位置
    let so = build_plugin(name, middleware_source, out_dir)?;
    // 原子写缓存：唯一 tmp（计数）→ rename，避免并发写同一 tmp 竞争（Bug 修复）
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let tmp = cache_path.with_extension(format!("tmp.{n}"));
    fs::copy(&so, &tmp).map_err(|e| format!("写缓存: {e}"))?;
    // 同名目标 rename 是原子的（Unix 覆盖），多线程同源码 → 内容一致，后写覆盖
    fs::rename(&tmp, &cache_path).map_err(|e| format!("缓存原子化: {e}"))?;

    // 临时 crate 已由 build_plugin_with_deps 成功路径清理（.so 在共享 target，
    // 不能从 .so 路径推导 crate_dir——会错删 out_dir/共享 target）
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
