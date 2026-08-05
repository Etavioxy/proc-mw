//! 数据驱动变体主程序：核心经共享 Node 链分发
#[path = "../traits.rs"]
mod traits;
#[path = "../mw_data.rs"]
mod mw_data;

include!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/gen/cores.rs"));
include!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/gen/data_run.rs"));

fn main() {
    std::hint::black_box(run_all());
}
