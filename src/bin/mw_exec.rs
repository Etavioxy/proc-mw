//! 子进程中间件执行器（D7 沙箱）：在独立进程中加载插件，经 stdin/stdout 通信。
//!
//! 两种协议：
//! - 默认（i32 文本）：宿主逐行发 `input`，执行器调用插件后逐行回 `processed`。
//! - `--bytes`（任意 repr(C)/POD）：`[u32 len LE][payload]` 发，插件原地变换，
//!   回 `[u8 status][u32 len LE][payload]`（status 0=ok / 1=拒绝）。
//!
//! 若插件 panic/abort（L3：extern C panic = 进程终止），**只有此进程死亡**，
//! 宿主（父进程）不受影响——这正是沙箱的价值。
//!
//! 用法：mw_exec <plugin.so> [--bytes]

use std::io::{BufRead, Read, Write};

use proc_mw::dispatch::Ctx;
use proc_mw::runtime::{Plugin, PluginOpaque};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let plugin_path = args.get(1).expect("usage: mw_exec <plugin.so> [--bytes]");
    let bytes_mode = args.iter().any(|a| a == "--bytes");

    if bytes_mode {
        byte_mode(plugin_path);
    } else {
        text_mode(plugin_path);
    }
}

/// i32 文本协议（原有）
fn text_mode(plugin_path: &str) {
    let plugin = Plugin::load(plugin_path).expect("加载插件");
    let node = plugin.to_node();
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        let input: i32 = line.trim().parse().unwrap_or(0);
        let mut ctx = Ctx::new(input);
        let out = match node.enter(&mut ctx) {
            Ok(_) => ctx.input,
            Err(_) => -999, // 错误哨兵
        };
        let mut so = stdout.lock();
        let _ = writeln!(so, "{out}");
        let _ = so.flush();
    }
}

/// 字节编组协议（任意 repr(C)/POD 类型沙箱）
fn byte_mode(plugin_path: &str) {
    let plugin = PluginOpaque::load(plugin_path).expect("加载插件");
    let stdin = std::io::stdin();
    let mut stdin = stdin.lock();
    let stdout = std::io::stdout();
    let mut stdout = stdout.lock();
    let mut len_buf = [0u8; 4];
    loop {
        // 读请求长度（EOF → 结束）
        if stdin.read_exact(&mut len_buf).is_err() {
            break;
        }
        let len = u32::from_le_bytes(len_buf) as usize;
        let mut payload = vec![0u8; len];
        if stdin.read_exact(&mut payload).is_err() {
            break;
        }
        // 插件原地变换（独立进程内）
        let code = unsafe {
            plugin.call(payload.as_mut_ptr() as *mut std::ffi::c_void, std::ptr::null_mut())
        };
        let status = if code == 0 { 0u8 } else { 1u8 };
        let _ = stdout.write_all(&[status]);
        let _ = stdout.write_all(&(payload.len() as u32).to_le_bytes());
        let _ = stdout.write_all(&payload);
        let _ = stdout.flush();
    }
}
