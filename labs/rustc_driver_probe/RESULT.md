# rustc_driver 集成 + 编译管线对比

> 2026-08-07 · nightly 1.99 · `labs/rustc_driver_probe/`

## 里程碑：rustc_driver（编译器作为库）编译任意 Rust 中间件成功

`full_pipeline`：rustc_driver 进程内编译中间件源码 → dlopen → proc-mw 链执行，**无 cargo 依赖**。两个不同中间件（×3 和 +10）验证成功。

## 编译管线对比（同一中间件源码，5 次）

| 管线 | 平均编译 | 产物大小 | 依赖 |
|---|---|---|---|
| **cargo 子进程**（build_plugin_cached） | 冷启动 ~0.12s / **缓存命中 0.00s** | **16 KB**（release 优化 + 可裁剪） | 需 cargo |
| **rustc_driver 内嵌** | **0.04s/次**（进程内，无子进程） | 420 KB（默认 debug 未优化） | 无 cargo，需 nightly |

## 关键发现

1. **rustc_driver 快**：0.04s/次 进程内编译，无子进程 spawn 开销——即使无缓存也比 cargo 冷启动快。
2. **产物大小差异 25×**：rustc_driver 默认 debug 未优化；**公平对比需传 `-C opt-level=3` + strip**（未在本次测量中体现）。
3. **取舍**：
   - 需要**无 cargo 依赖**（嵌入式/离线）→ rustc_driver
   - 需要**产物最小/缓存复用** → cargo 管线（build_plugin_cached）
   - 两者可共存：rustc_driver 为快速无依赖路径，cargo 为优化产物路径

## 复现

```bash
cd labs/rustc_driver_probe
cargo run --bin full_pipeline   # 完整链路（编译→dlopen→执行）
cargo run --bin compare         # 管线对比
```
