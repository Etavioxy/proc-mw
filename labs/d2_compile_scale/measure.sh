#!/bin/bash
# 预编译链编译/运行平衡实证：
# 1) N handler 共享 M 预编译函数（M=2，非 N）
# 2) 改 handler → 只 app 重编；改链 → 只 chains 重编，app 保持 Fresh
set -e
cd "$(dirname "$0")"
N=${1:-2000}
bash gen.sh "$N"
TARGET="target/release"

echo "========== 预编译链库（M=2 形状） + N=$N handler =========="
cargo clean -q -p app -p chains 2>/dev/null || true
echo "--- 干净构建 ---"
{ time cargo build --release -p app 2>/dev/null; } 2>&1 | grep real

echo "--- 改 1 个 handler（app 内）后增量 ---"
# 修改 handler_5 的 body（保持签名）
sed -i '' 's/pub fn handler_5(x: i32)/pub fn handler_5(x: i32)/; s/standard(x)/standard(x + 1)/' app/src/gen/handlers.rs
{ time cargo build --release -p app 2>/dev/null; } 2>&1 | grep real
sed -i '' 's/standard(x + 1)/standard(x)/' app/src/gen/handlers.rs

echo "--- 改链 body（chains 库内，接口不变）后增量 ---"
sed -i '' 's/if ctx.input > 50 {/if ctx.input > 51 {/' chains/src/lib.rs
{ time cargo build --release -p app 2>/dev/null; } 2>&1 | grep real
sed -i '' 's/if ctx.input > 51 {/if ctx.input > 50 {/' chains/src/lib.rs

echo ""
echo "--- 预编译链函数符号数（应 = M=2，不是 N=$N）---"
echo "standard 符号: $(nm -g $TARGET/app 2>/dev/null | grep -c standard)"
echo "light 符号:    $(nm -g $TARGET/app 2>/dev/null | grep -c light)"
echo "--- 二进制体积（对比 d5 泛型 1.28MB）---"
ls -la $TARGET/app | awk '{print $5, "bytes"}'
