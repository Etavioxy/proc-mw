//! 生产 dyn 对象的具体类型藏在库里——跨 crate 消费方看不到它。
//! 无 LTO 时 bin 无法去虚拟化；LTO 后全程序可见，LLVM 有机会内联。

/// 返回一个具体闭包，装箱为 dyn（具体类型对调用方隐藏）
pub fn make_mw() -> Box<dyn Fn(i32) -> i32> {
    Box::new(|x| x * 10 + 1)
}
