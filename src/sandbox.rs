//! D7 安全 · 子进程沙箱（evcxr 式隔离）
//!
//! 不可信插件（含动态编译的任意代码）跑在独立子进程中，经 stdin/stdout 通信。
//! 插件 panic/abort 时只杀子进程，宿主存活；可检测崩溃并重启沙箱。
//!
//! 协议：宿主发 `input` 行 → 子进程回 `processed` 行；EOF = 子进程死亡。

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::Mutex;

struct SandboxInner {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

/// 沙箱：持有子进程，`run` 经 IPC 调插件
pub struct Sandbox {
    inner: Mutex<SandboxInner>,
    exec_path: PathBuf,
    plugin_path: PathBuf,
}

impl Sandbox {
    /// 启动沙箱：`exec_path` = mw_exec 可执行文件，`plugin_path` = 插件 .so
    pub fn spawn(exec_path: &Path, plugin_path: &Path) -> Result<Self, String> {
        let mut child = Command::new(exec_path)
            .arg(plugin_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .map_err(|e| format!("spawn mw_exec: {e}"))?;
        let stdin = child.stdin.take().ok_or("取 stdin 失败")?;
        let stdout = BufReader::new(child.stdout.take().ok_or("取 stdout 失败")?);
        Ok(Sandbox {
            inner: Mutex::new(SandboxInner { child, stdin, stdout }),
            exec_path: exec_path.to_path_buf(),
            plugin_path: plugin_path.to_path_buf(),
        })
    }

    /// 发送输入并读回处理结果；子进程死亡（EOF）→ Err
    pub fn run(&self, input: i32) -> Result<i32, String> {
        let mut g = self.inner.lock().unwrap();
        writeln!(g.stdin, "{input}").map_err(|e| format!("写输入: {e}"))?;
        let mut line = String::new();
        match g.stdout.read_line(&mut line) {
            Ok(0) => Err("沙箱子进程崩溃/退出".to_string()), // EOF：子进程死了
            Ok(_) => {
                let v: i32 = line.trim().parse().map_err(|_| format!("坏输出: {line}"))?;
                if v == -999 {
                    Err("插件返回错误".to_string())
                } else {
                    Ok(v)
                }
            }
            Err(e) => Err(format!("读输出: {e}")),
        }
    }

    /// 崩溃后重启沙箱（新子进程）
    pub fn restart(&self) -> Result<(), String> {
        let mut g = self.inner.lock().unwrap();
        let mut new_child = Command::new(&self.exec_path)
            .arg(&self.plugin_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .map_err(|e| format!("重启 mw_exec: {e}"))?;
        g.stdin = new_child.stdin.take().ok_or("取 stdin 失败")?;
        g.stdout = BufReader::new(new_child.stdout.take().ok_or("取 stdout 失败")?);
        g.child = new_child;
        Ok(())
    }
}
