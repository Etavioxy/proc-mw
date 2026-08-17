# proc-mw

[English](README.md) | 中文

生产 Service 语义 × 运行期热重载的 Rust 中间件层——八维核心约束在同一个 crate 里持续验证。

## 这是什么

`proc-mw` 是一个 Rust 中间件系统：业务核心（纯函数）与横切逻辑（中间件）解耦，中间件在核心进入/退出时插入逻辑，支持运行时动态增删，并追求 **Release 零成本**——Debug 下全功能动态，Release 下剥离中间层、退化为裸函数调用。

设计以「最终植入大型 Rust 系统」为判据，八维约束在同一项目中持续验证：

| 维度 | 内容 | 落点 |
|---|---|---|
| D1 | 表达层·零成本抽象 | `src/lib.rs`（Proc trait + 遮蔽+cfg 装配） |
| D2 | 类型通道·零成本分发 | `src/dispatch.rs`（enum / fn 指针 / 位标记 / dyn / OpaqueNode） |
| D3 | 动态性·数据/快照增删 | `src/chain.rs`（RCU 快照，读路径无锁零分配） |
| D4 | 性能·局部加法+空链透明 | `examples/d4_bench.rs` |
| D5 | 编译层·LLVM 结构 + 真实动态链接 | `examples/` + `labs/` |
| D6 | 扩展形态·运行期加载 | `labs/` |
| D7 | 安全与定位 | `labs/` |
| D8 | 迁移工具链 | `labs/d8_migrate/` |

**核心目的**：`OpaqueNode` 用 `*mut c_void` 擦除类型——宿主与运行期编译的插件各自定义同一 `#[repr(C)]` 布局的共享类型，插件内部对任意类型调用方法，实现真正的类型开放 + 运行期热重载。

## 文档入口

- [`CORE-CONSTRAINTS.md`](CORE-CONSTRAINTS.md) — 八维核心约束定稿（凝练版）
- [`DESIGN-DRAFT.md`](DESIGN-DRAFT.md) — 设计探讨过程草稿（含纠错轨迹）
- [`docs/`](docs/) — 各维度的深入分析、极限评估与实测结果

## 如何探索

- **示例**：`examples/` 下的 `d1`~`d7` 系列覆盖每维度的可运行验证
- **实验台**：`labs/` 是测量台——合成代码生成、独立 dylib 编译单元、迁移工具链等隔离实验
- **测试**：`cargo test` 运行全量测试（含泛型通道、沙箱拒绝路径、超时边界等）

```bash
cargo build --release
cargo test
cargo run --example d4_bench --release   # 性能维度实测
```

## AI-generated

> This codebase was written with AI assistance — 100% AI-generated (with human review). It is evidence for quality-assured engineering with AI tooling.

## License

[MIT](LICENSE)
