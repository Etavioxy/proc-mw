# 工具链打包/分发域

> 运行期编译管线（核心目的）在生产部署的工具链需求与验证。

## 需求

运行期编译任意 Rust 中间件（`build_plugin`）**依赖本机有 cargo/rustc**（子进程调用）。生产部署需满足：

1. **cargo + rustc 可用**（PATH 或显式路径）
2. **--offline 可构建**：临时中间件 crate 无依赖，`--offline` 不联网
3. 工具链版本一致（中间件与宿主同 Rust 版本，ABI 契约）

## 验证

```rust
let r = proc_mw::compile::toolchain_report();
// r.usable == true → 运行期编译管线可用
// r.cargo / r.rustc → 版本信息
```

## 部署形态

| 形态 | 说明 |
|---|---|
| **内嵌工具链** | 打包 cargo/rustc 进产物（大，~1GB）；离线自足 |
| **旁挂编译服务** | 独立编译服务（cargo 在服务端），宿主经 IPC 请求编译 |
| **预编译产物分发** | 中间件预编译成 .so 分发，宿主只 dlopen（免工具链）——最小部署 |

## 环境限制

- **rustc_driver**（rustc 作为库内嵌）：需 nightly + rustc-dev 组件，当前 stable 工具链不可行（记 L-环境项）。
- 宿主语言 Rust 版本变化 → 插件 ABI 契约（`proc_mw_abi_version`）需同步，加载时校验。

## 结论

运行期编译管线是"编译服务"而非"纯运行时"——生产部署需工具链或预编译分发。`toolchain_report()` 提供部署前验证。
