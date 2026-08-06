//! D6 扩展形态 · 运行期动态编译 + 加载（evcxr 参考，非照搬）
//!
//! 宿主 dlopen 插件 .so，解析 `extern "C"` 函数指针 → `Plugin::to_node()`
//! 产出一个 **thin 的 `Node::Extern`**（无 vtable、无 Box<dyn>）。
//!
//! D6 落槽原则（不强制 dyn）：无状态插件 → thin `Extern`；只有**有状态**的开放
//! 插件才需要 `Dyn`。追求极致 = 每个槽位都验证（见 tests/d2_dispatch.rs 四槽位）。
//!
//! 拿 evcxr 的管线（dlopen → 符号解析 → 永不卸载），丢 REPL 壳。
//! ABI 契约（D7）：`extern "C"` + 版本符号校验 + 永不 unload（evcxr 教训）。

use std::any::Any;
use std::sync::Arc;

use crate::dispatch::{ExternNode, Node};

/// 插件 ABI 契约版本（加载时校验）
pub const PLUGIN_ABI_VERSION: i32 = 1;

/// 插件加载器：dlopen + 符号解析 + ABI 校验。加载后产出 Node::Extern。
pub struct Plugin {
    _lib: Arc<libloading::Library>, // 永不 unload；Arc 使保活句柄可克隆
    abi_version: i32,
    enter: unsafe extern "C" fn(*mut i32, *mut i32) -> i32,
    exit: Option<unsafe extern "C" fn(*mut i32)>,
}

unsafe impl Send for Plugin {}
unsafe impl Sync for Plugin {}

/// 跨平台符号解析：Linux/Windows 直查；macOS Mach-O 符号带 `_` 前缀，先直查再补 `_` 重试。
unsafe fn get_sym<'lib, T>(
    lib: &'lib libloading::Library,
    name: &[u8],
) -> Result<libloading::Symbol<'lib, T>, libloading::Error> {
    match lib.get(name) {
        Ok(s) => Ok(s),
        #[cfg(target_os = "macos")]
        Err(_) => {
            let mut prefixed = Vec::with_capacity(name.len() + 1);
            prefixed.push(b'_');
            prefixed.extend_from_slice(name);
            lib.get(prefixed.as_slice())
        }
        #[cfg(not(target_os = "macos"))]
        Err(e) => Err(e),
    }
}

impl Plugin {
    /// 从 .so/.dylib 路径加载插件（dlopen + 符号解析 + ABI 版本校验）
    pub fn load(path: &str) -> Result<Self, String> {
        unsafe {
            let lib = libloading::Library::new(path).map_err(|e| format!("dlopen({path}): {e}"))?;
            // Symbol 借用 lib，必须先取值（Copy）再移动 lib
            // ABI 版本用函数导出（fn 指针无符号尺寸问题，跨平台稳）
            let ver_fn = *get_sym::<unsafe extern "C" fn() -> i32>(&lib, b"proc_mw_abi_version")
                .map_err(|_| "缺 ABI 版本函数 proc_mw_abi_version".to_string())?;
            let ver = ver_fn();
            if ver != PLUGIN_ABI_VERSION {
                return Err(format!("ABI 版本不匹配：插件 {} ≠ 宿主 {}", ver, PLUGIN_ABI_VERSION));
            }
            let enter = *get_sym::<unsafe extern "C" fn(*mut i32, *mut i32) -> i32>(&lib, b"mw_enter")
                .map_err(|_| "缺 mw_enter 符号".to_string())?;
            let exit: Option<unsafe extern "C" fn(*mut i32)> =
                get_sym::<unsafe extern "C" fn(*mut i32)>(&lib, b"mw_exit")
                    .ok()
                    .map(|s| *s);
            Ok(Plugin {
                _lib: Arc::new(lib),
                abi_version: ver,
                enter,
                exit,
            })
        }
    }

    pub fn abi_version(&self) -> i32 {
        self.abi_version
    }

    /// 产出 thin 的 `Node::Extern`：函数指针 + 保活句柄（Arc<Library> 经类型擦除）
    pub fn to_node(&self) -> Node {
        Node::Extern(ExternNode {
            enter: self.enter,
            exit: self.exit,
            keepalive: Arc::clone(&self._lib) as Arc<dyn Any + Send + Sync>,
        })
    }
}

/// 共享类型布局指纹（D7）：`size<<32 | align`。宿主用它校验插件共享类型布局一致。
pub fn layout_fingerprint<T>() -> u64 {
    (std::mem::size_of::<T>() as u64) << 32 | (std::mem::align_of::<T>() as u64)
}

/// 富化布局指纹（D7）：FNV-1a 哈希 over 每字段的 `(offset, size, align)` 三元组
/// （按声明顺序）。
///
/// 捕获**同 size/align 的字段重排**——朴素 `size<<32|align` 测不到，且单纯哈希偏移
/// 也测不到（声明顺序下偏移恒为 0,8,...）。只有 (offset,size,align) 三元组才能区分
/// `{a:u64,b:u8}` 与 `{b:u8,a:u64}`（同为 16B/8 对齐但字段语义不同）。宿主用
/// `offset_of!`/`size_of!`/`align_of!` 提供自己的字段布局，插件在共享类型定义里导出
/// 同一哈希 → 漂移在加载期被拦截。
pub fn layout_fingerprint_of(fields: &[(usize, usize, usize)]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &(off, sz, al) in fields {
        for &x in [off, sz, al].iter() {
            h ^= x as u64;
            h = h.wrapping_mul(0x100000001b3);
        }
    }
    h
}

/// 类型无关插件加载器（核心目的：编译任意 Rust 代码粘合进中间层，L7）
///
/// ABI 用 `*mut c_void`（类型擦除指针）——插件在共享类型定义上编译，
/// downcast 后调用任意类型的方法（`String::push`/`Vec::sort`/struct 字段）。
/// 与 `Plugin`（i32 专用 ABI）的区别：此处宿主不感知具体类型，由插件契约决定。
pub struct PluginOpaque {
    _lib: Arc<libloading::Library>,
    abi_version: i32,
    enter: unsafe extern "C" fn(*mut std::ffi::c_void, *mut std::ffi::c_void) -> i32,
    #[allow(dead_code)] // exit 钩子：契约保留，当前示例未调用
    exit: Option<unsafe extern "C" fn(*mut std::ffi::c_void)>,
}

unsafe impl Send for PluginOpaque {}
unsafe impl Sync for PluginOpaque {}

impl PluginOpaque {
    pub fn load(path: &str) -> Result<Self, String> {
        unsafe {
            let lib = libloading::Library::new(path).map_err(|e| format!("dlopen({path}): {e}"))?;
            let ver_fn = *get_sym::<unsafe extern "C" fn() -> i32>(&lib, b"proc_mw_abi_version")
                .map_err(|_| "缺 ABI 版本函数 proc_mw_abi_version".to_string())?;
            let ver = ver_fn();
            if ver != PLUGIN_ABI_VERSION {
                return Err(format!("ABI 版本不匹配：插件 {} ≠ 宿主 {}", ver, PLUGIN_ABI_VERSION));
            }
            let enter = *get_sym::<unsafe extern "C" fn(*mut std::ffi::c_void, *mut std::ffi::c_void) -> i32>(
                &lib,
                b"mw_enter",
            )
            .map_err(|_| "缺 mw_enter 符号".to_string())?;
            let exit: Option<unsafe extern "C" fn(*mut std::ffi::c_void)> =
                get_sym::<unsafe extern "C" fn(*mut std::ffi::c_void)>(&lib, b"mw_exit")
                    .ok()
                    .map(|s| *s);
            Ok(PluginOpaque {
                _lib: Arc::new(lib),
                abi_version: ver,
                enter,
                exit,
            })
        }
    }

    pub fn abi_version(&self) -> i32 {
        self.abi_version
    }

    /// 调用插件的 enter（宿主侧 downcast 由插件契约决定）
    pub unsafe fn call(&self, req: *mut std::ffi::c_void, resp: *mut std::ffi::c_void) -> i32 {
        unsafe { (self.enter)(req, resp) }
    }

    /// 加载 + 布局指纹校验（D7）：插件须导出 `proc_mw_layout_fingerprint() -> u64`
    /// 且与宿主期望一致（size<<32|align）。共享类型定义漂移 → 加载期硬失败，而非运行期 UB。
    pub fn load_with_layout(path: &str, expected_layout: u64) -> Result<Self, String> {
        let p = Self::load(path)?;
        unsafe {
            let fp_fn =
                *get_sym::<unsafe extern "C" fn() -> u64>(&*p._lib, b"proc_mw_layout_fingerprint")
                    .map_err(|_| "缺 proc_mw_layout_fingerprint 符号".to_string())?;
            let fp = fp_fn();
            if fp != expected_layout {
                return Err(format!(
                    "共享类型布局指纹不匹配：插件 {fp:#x} ≠ 宿主 {expected_layout:#x}（类型定义漂移）"
                ));
            }
        }
        Ok(p)
    }

    /// 产出类型无关链节点（`opaque::OpaqueNode::Thin`）——把运行期编译的**任意类型**
    /// 中间件粘合进中间层（核心目的落地；区别于 `Plugin::to_node` 的 i32 控制面节点）。
    pub fn to_node(&self) -> crate::opaque::OpaqueNode {
        crate::opaque::OpaqueNode::Thin {
            enter: self.enter,
            exit: self.exit,
            keepalive: Arc::clone(&self._lib) as Arc<dyn Any + Send + Sync>,
        }
    }
}
