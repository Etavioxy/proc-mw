//! 实验：堆类型沙箱编组（marshalling 契约）— String 经固定缓冲跨子进程
//!
//! 纠正"堆类型跨进程物理失效"边界：堆 String 编组为固定内联缓冲（无指针），
//! 可经字节沙箱。宿主编组 → 子进程插件变换 → 解编组。
//!
//! 跑：`cd labs/scales/large && cargo run --release --bin exp_sandbox_marshalling`

use proc_mw::compile::build_plugin_cached;
use proc_mw::sandbox::Sandbox;

/// 编组契约（宿主与插件共享布局）：[u64 id][u32 text_len][u8;64 文本缓冲]
#[repr(C)]
#[derive(Clone, Copy)]
struct Marshalled {
    id: u64,
    text_len: u32,
    text: [u8; 64],
}

impl Marshalled {
    fn marshal(id: u64, text: &str) -> Marshalled {
        let mut m = Marshalled { id, text_len: 0, text: [0u8; 64] };
        let b = text.as_bytes();
        m.text_len = b.len().min(64) as u32;
        m.text[..m.text_len as usize].copy_from_slice(&b[..m.text_len as usize]);
        m
    }
    fn text(&self) -> String {
        String::from_utf8_lossy(&self.text[..self.text_len as usize]).to_string()
    }
}

fn main() {
    // 编译编组插件（自包含，定义 marshalling 契约）
    let src = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/exp_sandbox_marshalling/mw_v1.rs"));
    let so = build_plugin_cached("sandbox_marshal", src, &std::env::temp_dir()).unwrap();

    let exec = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../../target/release/mw_exec"));
    let sb = Sandbox::spawn_bytes(exec, &so).unwrap();

    // 编组 Msg{id, text:"caw"} → 子进程插件变换 → 解编组
    let msg = Marshalled::marshal(7, "caw");
    let bytes: &[u8] = unsafe {
        std::slice::from_raw_parts((&msg as *const Marshalled) as *const u8, std::mem::size_of::<Marshalled>())
    };
    let out = sb.run_bytes(bytes).expect("沙箱处理");
    let out_msg: Marshalled = unsafe { std::ptr::read(out.as_ptr() as *const Marshalled) };
    println!("[1] 编组沙箱：Msg{{id:7, text:\"caw\"}} → id={} text=\"{}\"（期望 id=7 text=\"caw-proc\"）",
        out_msg.id, out_msg.text());
    assert_eq!(out_msg.id, 7);
    assert_eq!(out_msg.text(), "caw-proc", "堆 String 经编组契约跨子进程变换");
    println!("---");
    println!("实验通过：堆类型沙箱编组（marshalling 契约纠正'物理失效'边界）✓");
}
