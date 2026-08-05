//! 核心契约：每个生成的核心类型实现它
pub trait Core {
    fn run(x: i32) -> i32;
}
