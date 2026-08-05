//! 泛型中间件库（反事实变体）。
//! 泛型接口——改这里（即使保持签名）会触发依赖 crate 中所有 Log<C_i> 单态化实例重编。

pub trait Core {
    fn run(x: i32) -> i32;
}

pub struct Log<C: Core> {
    pub core: C,
    pub tag: i32,
}
impl<C: Core> Log<C> {
    #[inline(never)]
    pub fn run(&self, x: i32) -> i32 {
        let y = C::run(x);
        y + self.tag
    }
}
