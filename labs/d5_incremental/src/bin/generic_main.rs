//! 泛型变体主程序：N 个核心各自被 `Log<C_i>` 单态化
#[path = "../traits.rs"]
mod traits;
#[path = "../mw_generic.rs"]
mod mw_generic;

include!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/gen/cores.rs"));
include!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/gen/generic_run.rs"));

fn main() {
    std::hint::black_box(run_all());
}
