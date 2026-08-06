#!/bin/bash
# 生成多 crate 大型系统：10 个领域 crate（各 ~5.5k LOC）+ app
set -e
cd "$(dirname "$0")"
DOMAINS=10
HANDLERS_PER_DOMAIN=500
mkdir -p domains app/src

# 每个领域 crate：500 handler（~5.5k LOC）
for d in $(seq 0 $((DOMAINS-1))); do
  mkdir -p domains/domain_$d/src
  cat > domains/domain_$d/Cargo.toml <<EOF
[package]
name = "domain_$d"
version = "0.1.0"
edition = "2021"
EOF
  {
    echo "pub fn handler_${d}_0(input: i32) -> i32 { input.wrapping_add($d + 1) }"
    for h in $(seq 1 $((HANDLERS_PER_DOMAIN-1))); do
      echo "pub fn handler_${d}_${h}(input: i32) -> i32 {"
      echo "    let base = input.wrapping_mul(($d + 1) as i32);"
      echo "    let factor = (($h % 7) + 3) as i32;"
      echo "    let s = base.wrapping_add(factor).wrapping_mul(factor);"
      echo "    s.wrapping_sub(($d * 11 + $h) as i32).wrapping_rem(97)"
      echo "}"
    done
  } > domains/domain_$d/src/lib.rs
done

# app：引用所有领域 + 中间件链
cat > app/Cargo.toml <<EOF
[package]
name = "large_app"
version = "0.1.0"
edition = "2021"

[dependencies]
proc-mw = { path = "../../../..", features = ["runtime"] }
EOF
for d in $(seq 0 $((DOMAINS-1))); do
  echo "domain_$d = { path = \"../domains/domain_$d\" }" >> app/Cargo.toml
done
echo "生成 $DOMAINS 领域 crate × $HANDLERS_PER_DOMAIN handler + app"
