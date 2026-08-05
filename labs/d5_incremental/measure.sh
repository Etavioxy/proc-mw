#!/bin/bash
# D5 增量失效测量：泛型中间件 vs 数据驱动中间件（跨 crate）
# 关键：改中间件 BODY 但保持接口不变 → 看依赖 crate 是否重编
set -e
cd "$(dirname "$0")"
N=${1:-400}
bash gen.sh "$N"
TARGET="../target/release"

echo "========== 泛型变体（Log<C_i> 在应用 crate 单态化） =========="
cargo clean -q -p app_generic -p mw_generic 2>/dev/null || true
echo "--- 干净构建 ---"
{ time cargo build --release -p app_generic 2>/dev/null; } 2>&1 | grep real
sed -i '' 's/y + self.tag/y.wrapping_add(self.tag)/' mw_generic/src/lib.rs
echo "--- 改 mw_generic body（接口不变）后增量 ---"
{ time cargo build --release -p app_generic 2>/dev/null; } 2>&1 | grep real
sed -i '' 's/y.wrapping_add(self.tag)/y + self.tag/' mw_generic/src/lib.rs

echo ""
echo "========== 数据驱动变体（共享 Node 链，非泛型） =========="
cargo clean -q -p app_data -p mw_data 2>/dev/null || true
echo "--- 干净构建 ---"
{ time cargo build --release -p app_data 2>/dev/null; } 2>&1 | grep real
sed -i '' 's/*x += k;/*x = x.wrapping_add(k);/' mw_data/src/lib.rs
echo "--- 改 mw_data body（接口不变）后增量 ---"
{ time cargo build --release -p app_data 2>/dev/null; } 2>&1 | grep real
sed -i '' 's/*x = x.wrapping_add(k);/*x += k;/' mw_data/src/lib.rs

echo ""
echo "--- 二进制体积 ---"
echo "generic: $(ls -la $TARGET/app_generic 2>/dev/null | awk '{print $5}') bytes"
echo "data:    $(ls -la $TARGET/app_data 2>/dev/null | awk '{print $5}') bytes"
echo "--- Log 单态化实例数（generic 应 ≈N）vs run_chain（data 应 =1）---"
echo "generic Log 符号: $(nm -g $TARGET/app_generic 2>/dev/null | grep -c 'Log')"
echo "data run_chain 符号: $(nm -g $TARGET/app_data 2>/dev/null | grep -c 'run_chain')"
