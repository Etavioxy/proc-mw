//! 反事实变体：泛型中间件。
//! 每个 `Log<C_i>` 是独立单态化实例——改这里会重编所有引用它的核心。
use crate::traits::Core;

pub struct Log<C: Core> {
    pub core: C,
    pub tag: i32,
}
impl<C: Core> Log<C> {
    #[inline(never)]
    pub fn run(&self, x: i32) -> i32 {
        // Core::run 是关联函数（无 self），经类型参数调用
        let y = C::run(x);
        y + self.tag
    }
}
// touched 1785953340
