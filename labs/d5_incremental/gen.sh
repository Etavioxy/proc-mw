#!/bin/bash
# 生成 N 个核心 + 每核心独立 handler，写入两个应用 crate 的 src/gen/
set -e
cd "$(dirname "$0")"
N=${1:-400}
mkdir -p app_data/src/gen app_generic/src/gen

# ---- app_data：cores 用固有方法（不需要 trait）；use 由 main.rs 提供 ----
{
  for i in $(seq 0 $((N-1))); do
    echo "pub struct C$i; impl C$i { pub fn run(x: i32) -> i32 { x.wrapping_add($i) } }"
  done
} > app_data/src/gen/cores.rs

{
  echo "pub static CHAIN: &[Node] = &[Node::Add(1)];"
  for i in $(seq 0 $((N-1))); do
    echo "#[inline(never)] pub fn handler_$i(x: i32) -> i32 { let y = C$i::run(x); run_chain(CHAIN, y) }"
  done
  echo "pub fn run_all() -> i32 {"
  echo "    let mut acc = 0i32;"
  for i in $(seq 0 $((N-1))); do
    echo "    acc = acc.wrapping_add(handler_$i(1));"
  done
  echo "    acc"
  echo "}"
} > app_data/src/gen/handlers.rs

# ---- app_generic：cores 实现 mw_generic::Core trait；use 由 main.rs 提供 ----
{
  for i in $(seq 0 $((N-1))); do
    echo "pub struct C$i; impl Core for C$i { fn run(x: i32) -> i32 { x.wrapping_add($i) } }"
  done
} > app_generic/src/gen/cores.rs

{
  for i in $(seq 0 $((N-1))); do
    echo "#[inline(never)] pub fn handler_$i(x: i32) -> i32 { Log { core: C$i, tag: 1 }.run(x) }"
  done
  echo "pub fn run_all() -> i32 {"
  echo "    let mut acc = 0i32;"
  for i in $(seq 0 $((N-1))); do
    echo "    acc = acc.wrapping_add(handler_$i(1));"
  done
  echo "    acc"
  echo "}"
} > app_generic/src/gen/handlers.rs

echo "generated N=$N into app_data & app_generic"
