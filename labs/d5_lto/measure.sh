#!/bin/bash
# LTO 对 dyn 去虚拟化的实测：无 LTO / thin / fat 三种构建，比较 run() 的间接调用
# 用 cargo --config 走 profile.lto（避免 RUSTFLAGS 与 embed-bitcode 冲突）
set -e
cd "$(dirname "$0")"

check() {
    local label="$1"; local lto="$2"
    cargo clean -q -p app -p dynlib 2>/dev/null
    cargo build --release -p app --config "profile.release.lto=\"$lto\"" 2>/dev/null \
        || { echo "[$label] 构建失败"; return; }
    cargo rustc --release -p app --bin app --config "profile.release.lto=\"$lto\"" -- --emit=asm 2>/dev/null
    local asm=$(ls -t target/release/deps/app-*.s 2>/dev/null | head -1)
    local body=$(sed -n '/3run17h/,/cfi_endproc/p' "$asm")
    local blr=$(echo "$body" | grep -c 'blr')
    local mul=$(echo "$body" | grep -c 'mul')
    echo "[$label] run() 内 blr(间接调用)=$blr, mul(直算)=$mul → $([ "$blr" -eq 0 ] && echo 去虚拟化 || echo 未去虚拟化)"
}

echo "=== 跨 crate dyn 调用（具体类型隐藏在 dynlib） ==="
check "无LTO" "off"
check "LTO=thin" "thin"
check "LTO=fat" "fat"

echo ""
echo "--- LTO=fat 下 run() 循环体（应直算，无 blr）---"
cargo clean -q -p app -p dynlib 2>/dev/null
cargo rustc --release -p app --bin app --config "profile.release.lto=\"fat\"" -- --emit=asm 2>/dev/null
asm=$(ls -t target/release/deps/app-*.s | head -1)
sed -n '/3run17h/,/cfi_endproc/p' "$asm" | grep -E '^\s+[a-z]' | head -12
