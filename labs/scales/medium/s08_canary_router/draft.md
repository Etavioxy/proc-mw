# 场景 S08 · 独立草稿 —— 灰度分流（直接依赖宿主 crate）

## 用户故事
> 作为**发布**，我想**按请求特征在 v1/v2 后端间分流并运行期调整比例**，以便**灰度发布零停机**。

## 设计决策
1. **直接依赖宿主 crate（usergoals 实证）**：插件 Cargo.toml 声明
   `medium_service = { path = <宿主> }`，源码 `use medium_service::CanaryReq`——
   **零手写共享类型**。这是 bevy 档的可行路径（类型放宿主/bevy，插件依赖）。
2. **分流决策写入请求字段**：`CanaryReq.route_to_v2`（插件写入），内层按此路由。
3. **比例热更**：v1 `user_id%10==0`（10%）→ v2 `user_id%2==0`（50%），`chain.set` 热替换。
4. 对比：S01-S07 用 `shared_types`（演示脚手架，62 LOC）；**S08 用直接依赖（0 LOC）**。

## 待测试点
- [x] v1 10% 灰度（2/20 到 v2）。
- [x] 热换 v2 50%（10/20 到 v2）。
- [x] 插件直接依赖宿主 crate（零 shared_types）。
- [ ] 按用户特征（非哈希）分流。

## 边界反思
- 直接依赖 vs shared_types：当类型在宿主 crate 中，直接依赖最优；若宿主 crate 无法
  作为依赖（循环/太重），才需提取 types crate（D8 迁移，机械复制非手写）。
- 分流失效需默认安全（route_to_v2 默认 false → 全量 v1）。
