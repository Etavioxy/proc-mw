//! D7 安全 · 子进程沙箱（evcxr 式隔离）
//!
//! 不可信插件（含动态编译的任意代码）跑在独立子进程中，经 stdin/stdout 通信。
//! 插件 panic/abort 时只杀子进程，宿主存活；可检测崩溃并重启沙箱。
//!
//! 协议：宿主发 `input` 行 → 子进程回 `processed` 行；EOF = 子进程死亡。

use std::io::{BufRead, BufReader, Read, Write};
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
    bytes_mode: bool,
}

impl Sandbox {
    /// 启动沙箱：`exec_path` = mw_exec 可执行文件，`plugin_path` = 插件 .so（i32 文本协议）
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
            bytes_mode: false,
        })
    }

    /// 启动字节编组沙箱（`mw_exec --bytes`）：任意 repr(C)/POD 类型经 `run_bytes` 运行
    pub fn spawn_bytes(exec_path: &Path, plugin_path: &Path) -> Result<Self, String> {
        let mut child = Command::new(exec_path)
            .arg(plugin_path)
            .arg("--bytes")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .map_err(|e| format!("spawn mw_exec --bytes: {e}"))?;
        let stdin = child.stdin.take().ok_or("取 stdin 失败")?;
        let stdout = BufReader::new(child.stdout.take().ok_or("取 stdout 失败")?);
        Ok(Sandbox {
            inner: Mutex::new(SandboxInner { child, stdin, stdout }),
            exec_path: exec_path.to_path_buf(),
            plugin_path: plugin_path.to_path_buf(),
            bytes_mode: true,
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

    /// 字节编组运行（任意 repr(C)/POD 类型沙箱）：把请求原始字节送进子进程，
    /// 插件在**独立进程**内原地变换，回传变换后字节。
    /// 协议：`[u32 len LE][payload]` 发；`[u8 status][u32 len LE][payload]` 收
    /// （status 0=ok / 1=拒绝；EOF=子进程崩溃）。
    /// 边界（D7 显式）：堆类型（String/Vec/Box）含进程内指针，跨进程编组无效——
    /// 字节沙箱仅适用 repr(C)/POD；堆类型走信任模型 + 返回码契约。
    pub fn run_bytes(&self, input: &[u8]) -> Result<Vec<u8>, String> {
        let mut g = self.inner.lock().unwrap();
        g.stdin
            .write_all(&(input.len() as u32).to_le_bytes())
            .map_err(|e| format!("写长度: {e}"))?;
        g.stdin.write_all(input).map_err(|e| format!("写 payload: {e}"))?;
        g.stdin.flush().map_err(|e| format!("flush: {e}"))?;
        let mut status = [0u8; 1];
        g.stdout
            .read_exact(&mut status)
            .map_err(|_| "沙箱子进程崩溃/退出".to_string())?;
        let mut len_buf = [0u8; 4];
        g.stdout
            .read_exact(&mut len_buf)
            .map_err(|_| "沙箱响应中断".to_string())?;
        let len = u32::from_le_bytes(len_buf) as usize;
        let mut payload = vec![0u8; len];
        g.stdout
            .read_exact(&mut payload)
            .map_err(|_| "沙箱响应不完整".to_string())?;
        if status[0] == 0 {
            Ok(payload)
        } else {
            Err("插件拒绝/错误".to_string())
        }
    }

    /// 崩溃后重启沙箱（新子进程）
    pub fn restart(&self) -> Result<(), String> {
        let mut g = self.inner.lock().unwrap();
        let mut cmd = Command::new(&self.exec_path);
        cmd.arg(&self.plugin_path);
        if self.bytes_mode {
            cmd.arg("--bytes");
        }
        let mut new_child = cmd
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
