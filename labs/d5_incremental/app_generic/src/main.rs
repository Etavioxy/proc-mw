//! 泛型应用：5000 个核心各自被 Log<C_i> 单态化
use mw_generic::{Core, Log};

include!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/gen/cores.rs"));
include!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/gen/handlers.rs"));

fn main() {
    std::hint::black_box(run_all());
}
