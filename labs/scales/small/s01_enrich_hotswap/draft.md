# 场景 S01 · 独立草稿 —— 中间热更连接（flume 泛型通道 × 全类型无关中间件层）

## 用户故事
> 作为**数据管道运维**，我想**在不重启、不停通道的前提下，修改消息进入 flume 通道前的
> 富化逻辑（路由标签/词法变换）**，以便**策略即时生效、生产者零停机**。

## 设计决策

1. **为什么 flume 做载体**：`flume::Sender<T>` 是泛型通道——消息类型天然任意，正是
   核心目的"任意类型进中间层"的真实软件锚点（tier small = flume 0.11，2,188 LOC）。
2. **为什么全类型无关链**：i32 的 `Ctx` 链只能治理 i32 请求；消息是 `Message` struct
   （含 `String` 变换），必须走 `OpaqueChain`。治理（metrics/限流）逻辑本就不碰
   Ctx 字段，迁移为 `OpaqueMw`（Stateful 槽位）后与变换（Thin 槽位）同链。
3. **共享类型定义**：宿主与运行期编译插件各自定义同一 `#[repr(C)] struct Message`，
   `c_void` 传递；编译期布局守卫（size==96）防漂移。
4. **热更机制**：`build_plugin_cached`（FNV 哈希缓存）→ `PluginOpaque::load` →
   `to_node()` → `chain.set(3, v2)`（RCU 快照替换，不停机）。
5. **中间件联调（≥3）**：OpaqueMetrics（治理）· OpaqueRateLimiter（治理）·
   ttl_drop（宿主 Thin）· enrich v1→v2（运行期编译插件，热更本体）。

## 契约（D7）
- 插件导出 `proc_mw_abi_version()`（版本符号校验）与 `mw_enter(req: *mut c_void, resp: *mut c_void) -> i32`。
- 返回码：0 继续 / 1 短路 / 2 拒绝；插件永不 panic，错误经返回码传播。
- 热替换永不 unload（`keepalive` 保活句柄）。

## 待测试点
- [x] 运行期编译任意 Rust（String/Vec/struct 字段）中间件，加载进链生效。
- [x] v1→v2 热替换行为可观测变化（ROUTE:A → ROUTE:B），通道不停。
- [x] 治理层（metrics/限流）在任意类型链上计数正确（calls=10/successes=10/errors=0）。
- [x] 消息为共享 repr(C) struct，布局守卫生效。
- [ ] 限流超配额时消息被拒、通道侧感知（下一场景 S02）。
- [ ] 失败重投（S03）/ 过滤熔断（S04）在任意类型链上。

## 边界反思（追求极致）
- 治理层 i32 锚定已消除：metrics/限流/熔断全部类型无关（`opaque_gov.rs` +
  `CircuitBreaker::call_opaque`）。
- 剩余边界：插件与宿主共享类型目前靠"同一 repr(C) 定义 + 布局守卫"手工保证；真正
  的"共享类型定义"应由编译管线注入（运行期把类型定义拼进插件 crate），避免双写漂移
  ——这是 D6 编译管线下一处要推的边。
- `dyn Trait`/闭包跨 .so 不可行（胖指针 16B > c_void 8B），是显式边界。
