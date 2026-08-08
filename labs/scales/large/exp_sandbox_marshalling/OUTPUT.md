# 实验 · 堆类型沙箱编组 · 输出

> 运行：`cd labs/scales/large && cargo run --release --bin exp_sandbox_marshalling`

## 运行输出（2026-08-09）

```
[1] 编组沙箱：Msg{id:7, text:"caw"} → id=7 text="caw-proc"
实验通过：堆类型沙箱编组（marshalling 契约纠正'物理失效'边界）✓
```

## 证据解读

| 证据 | 含义 | 维度 |
|---|---|---|
| 堆 String 经固定内联缓冲跨子进程 | **marshalling 契约**（无指针布局）纠正"堆类型物理失效"边界 | D7 |
| 插件从缓冲重建 String、变换、写回 | 堆类型可被沙箱化（拷贝语义，非零拷贝） | D7/D6 |
| id=7 保持 / text 变换 | 编组往返完整 | D7 |
| **边界修正**：零拷贝不行 → 编组行（拷贝为隔离的正确代价） | 从"根本边界"升级为"marshalling 契约" | D7 |
