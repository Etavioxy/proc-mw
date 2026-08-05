# D5 编译层 · LTO 对 dyn 去虚拟化的实测

> 日期：2026-08-06 · Apple Silicon · `labs/d5_lto/measure.sh`

## 结论

**CORE-CONSTRAINTS D5 承诺"dyn 仅 ThinLTO+PGO 全程序信息下才可能去虚拟化"——实测成立。** 跨 crate 的 dyn 调用：无 LTO 保持间接调用（blr），LTO 开启后去虚拟化 + 内联，fat 下还触发自动向量化。

## 实测（run() 循环体）

| 构建 | blr（间接调用） | mul（直算） | 判定 |
|---|---|---|---|
| 无 LTO | **2** | 0 | **未去虚拟化** |
| LTO=thin | 0 | 6 | 去虚拟化 + 内联 |
| LTO=fat | 0 | 4 | 去虚拟化 + 内联 + **SIMD 向量化** |

## 为什么

具体类型藏在 `dynlib::make_mw()`（跨 crate），无 LTO 时 bin 看不到 → 只能经 vtable 间接调用。
LTO 后全程序可见 → LLVM 内联 make_mw → 具体闭包暴露 → 去虚拟化 + 内联（甚至向量化）。

## 判定

- **D5 承诺验证 ✓**：dyn 去虚拟化确实需要 LTO（全程序信息）。PGO 未测（本 lab 仅验证 LTO 维度）。
- **设计含义**：
  - 需要 dyn 去虚拟化性能 → 必须开 LTO（生产构建标配）。
  - D2 的"enum 必去虚拟化"与"dyn 需 LTO"形成对比：enum 无 LTO 也去虚拟化（csel 直排，D2 已证），dyn 依赖构建配置。

## 复现

```bash
bash labs/d5_lto/measure.sh
```
