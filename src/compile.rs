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
        return Err(format!(
            "cargo 编译失败: {}",
            String::from_utf8_lossy(&out.stderr)
        ));
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
