//! N 个 handler 共享预编译链：每个 handler = 1 次对 chains::standard/light 的直调
include!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/gen/handlers.rs"));

fn main() {
    std::hint::black_box(run_all());
}
