//! 探针：验证 bevy_ecs 事件机制（System 参数方式）可编译（S01 前置）
use bevy_ecs::event::{Event, EventReader, EventWriter, Events};
use bevy_ecs::prelude::*;
use bevy_ecs::schedule::Schedule;

#[derive(Event, Clone, Debug)]
struct InputEvent {
    key: u8,
    pos: (f32, f32),
}

#[derive(Resource, Default)]
struct Received(u32);

fn producer(mut w: EventWriter<InputEvent>) {
    w.send(InputEvent { key: 1, pos: (0.0, 0.0) });
    w.send(InputEvent { key: 2, pos: (1.0, 1.0) });
}

fn consumer(mut r: EventReader<InputEvent>, mut recv: ResMut<Received>) {
    for _ in r.read() {
        recv.0 += 1;
    }
}

fn main() {
    let mut world = World::new();
    world.init_resource::<Events<InputEvent>>();
    world.init_resource::<Received>();
    let mut schedule = Schedule::default();
    schedule.add_systems((producer, consumer));
    schedule.run(&mut world);
    schedule.run(&mut world); // 第二帧：事件被消费
    let n = world.resource::<Received>().0;
    println!("bevy_ecs 事件机制可用：收到 {n} 条事件");
    assert!(n >= 2, "事件应被消费者系统读取");
}
