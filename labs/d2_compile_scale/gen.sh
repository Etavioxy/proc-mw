#!/bin/bash
# 生成 N 个 handler，各调用预编译链 standard 或 light（共享，非每核心单态化）
set -e
cd "$(dirname "$0")"
N=${1:-2000}
mkdir -p app/src/gen

{
  echo "use chains::{standard, light};"
  for i in $(seq 0 $((N-1))); do
    if [ $((i % 2)) -eq 0 ]; then
      echo "#[inline(never)] pub fn handler_$i(x: i32) -> Result<i32, proc_mw::dispatch::MwError> { standard(x) }"
    else
      echo "#[inline(never)] pub fn handler_$i(x: i32) -> Result<i32, proc_mw::dispatch::MwError> { light(x) }"
    fi
  done
  echo "pub fn run_all() -> i32 {"
  echo "    let mut acc = 0i32;"
  for i in $(seq 0 $((N-1))); do
    echo "    acc = acc.wrapping_add(handler_$i(1).unwrap_or(0));"
  done
  echo "    acc"
  echo "}"
} > app/src/gen/handlers.rs
echo "generated N=$N handlers into app/src/gen/"
