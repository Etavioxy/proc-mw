# 实验 01 · D1 表达层 · 零成本抽象 — 验证结果

> 日期：2026-08-06 · 工具链：rustc/cargo 1.94.1 · 平台：Apple Silicon (ARM64)

## 结论：D1 达到极致 ✓

**包装路径与裸调用路径在 Release 下符号级不可区分——编译器直接折叠合并，无法分辨两者的存在。**

## 验证的多重测试

### T1 · 行为一致性（4 样本 × 双路径）
样本 `x ∈ {-10, 0, 5, 1000}`，Debug 与 Release 双构建各跑一遍：
- `through_pipeline(x) == direct_bare(x) == x + 1` 全部通过
- 核心语义 `x+1` 在包裹前后**不变**（中间件只附加观测，不改变语义）

### T2 · 内存足迹
- **Debug**：`size_of(pipeline) = 16B`（携带 `&str tag`，中间件状态真实存在）
- **Release**：`size_of(pipeline) = 0B`（退化为 ZST `Add`，零占用）

### T3 · 构建模式标记
- **Debug**：打印 4 组 `enter/exit` 日志（中间件生效）
- **Release**：无任何日志输出，且编译告警 `struct Log is never constructed` —— **cfg 词法层剥离的直接证据**（编译器根本看不到 `Log` 被使用）

### T4 · 机器码验证（`cargo rustc --release -- --emit=asm`）
Release 汇编中的函数符号**只有两个**：

```
__ZN13d1_expression11direct_bare...:   # 裸调用
	add	w0, w0, #1
	ret

__ZN13d1_expression4main...:           # 主程序
	bl  direct_bare   ×8   ← 4 次裸调用 + 4 次包装调用全部折叠到这里
	bl  _print        ×5
	bl  assert_failed ×2
```

**`through_pipeline` 符号在整个 .s 文件中出现 0 次。**
原因：Release 下 `build_pipeline()`（`#[inline(always)]`）展开为 `Add`（ZST），`Add::exec` = `x+1`，于是 `through_pipeline` 的 body 与 `direct_bare` 逐字节相同 → LLVM 把 4 次包装调用重定向到 `direct_bare` 并删除原符号。

## 对 D1 三项承诺的判定

| 承诺 | 判定 | 证据 |
|---|---|---|
| 1. 业务核心零污染 | ✓ | `Add` 源码只有 `x + 1`，无 cfg/println |
| 2. 装配语义直白（遮蔽+cfg） | ✓ | Debug 套壳 / Release 词法层消失（dead_code 告警） |
| 3. Release 机器码级等价 | ✓ **（超预期：符号级等价）** | 包装符号被消除，8 次调用全折叠进裸调用 |

## 复现

```bash
cd proc-mw-experiment
cargo run -p d1_expression                 # Debug：看日志 + 16B
cargo run -p d1_expression --release       # Release：无日志 + 0B
cargo rustc -p d1_expression --release -- --emit=asm
# 反汇编 main 确认 bl 目标只有 direct_bare / _print / assert_failed
```

## 一句话

D1 的"机器码级等价"实测比纸面更强——**编译器不仅把抽象优化掉了，而是彻底认不出它与裸调用的区别**。这为后续维度（D2 分发、D3 快照、D4 空链透明）确立了"零成本必须证明到符号级"的验收标准。
