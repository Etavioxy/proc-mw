#!/bin/bash
# 生成中型系统：10 模块 × 50 handler，每 handler ~20 LOC → ~万 LOC
set -e
cd "$(dirname "$0")"
MODS=${1:-10}
HANDLERS_PER_MOD=${2:-50}
mkdir -p src/gen

{
  echo "// 生成：${MODS} 模块 × ${HANDLERS_PER_MOD} handler（中型系统，~万 LOC）"
  echo "pub const HANDLERS: &[fn(i32) -> i32] = &["
  for m in $(seq 0 $((MODS-1))); do
    for h in $(seq 0 $((HANDLERS_PER_MOD-1))); do
      echo "  handler_${m}_${h},"
    done
  done
  echo "];"
  echo ""
  for m in $(seq 0 $((MODS-1))); do
    for h in $(seq 0 $((HANDLERS_PER_MOD-1))); do
      # 每 handler ~20 LOC 的业务逻辑（本地变量 + 变换）
      echo "pub fn handler_${m}_${h}(input: i32) -> i32 {"
      echo "    let base = input.wrapping_mul(($m + 1) as i32);"
      echo "    let factor = (($h % 7) + 3) as i32;"
      echo "    let step1 = base.wrapping_add(factor);"
      echo "    let step2 = step1.wrapping_mul(factor);"
      echo "    let step3 = step2.wrapping_sub(($m * 11 + $h) as i32);"
      echo "    let step4 = step3.wrapping_rem(97);"
      echo "    let step5 = step4.wrapping_add(($h % 5) as i32);"
      echo "    step5"
      echo "}"
    done
  done
} > src/gen.rs
echo "生成 ${MODS} 模块 × ${HANDLERS_PER_MOD} handler，源码行数：$(wc -l < src/gen.rs)"
