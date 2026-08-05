//! 数据驱动应用：5000 个 handler 经共享 Node 链
use mw_data::{Node, run_chain};

include!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/gen/cores.rs"));
include!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/gen/handlers.rs"));

fn main() {
    std::hint::black_box(run_all());
}
