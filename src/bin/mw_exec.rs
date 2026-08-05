//! 子进程中间件执行器（D7 沙箱）：在独立进程中加载插件，经 stdin/stdout 通信。
//!
//! 协议：宿主逐行发 `input`，执行器调用插件 mw_enter 后逐行回 `processed`。
//! 若插件 panic/abort（L3：extern C panic = 进程终止），**只有此进程死亡**，
//! 宿主（父进程）不受影响——这正是沙箱的价值。
//!
//! 用法：mw_exec <plugin.so>

use std::io::{BufRead, Write};

use proc_mw::dispatch::Ctx;
use proc_mw::runtime::Plugin;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let plugin_path = args.get(1).expect("usage: mw_exec <plugin.so>");
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
