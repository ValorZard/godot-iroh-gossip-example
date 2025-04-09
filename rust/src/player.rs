use std::sync::Arc;
use std::sync::Mutex;

use godot::classes::ISprite2D;
use godot::classes::Sprite2D;
use godot::obj::BaseMut;
use godot::prelude::*;

use crate::async_singleton::AsyncSingleton;

#[derive(GodotClass)]
#[class(base=Sprite2D)]
struct Player {
    speed: f64,
    angular_speed: f64,

    base: Base<Sprite2D>,
}

#[godot_api]
impl Player {
    #[signal]
    fn damage_taken(amount: i32);

    fn on_damage_taken(&mut self, amount: i32) {
        godot_print!("Damage taken: {}", amount);
    }
}

#[godot_api]
impl ISprite2D for Player {
    fn init(base: Base<Sprite2D>) -> Self {
        godot_print!("Hello, world!"); // Prints to the Godot console

        Self {
            speed: 400.0,
            angular_speed: std::f64::consts::PI,
            base,
        }
    }

    fn ready(&mut self) {
        self.signals()
            .damage_taken()
            .connect_self(Self::on_damage_taken);

        // Call the async event bus singleton method
        let async_singleton = self
            .base()
            .get_tree()
            .unwrap()
            .get_root()
            .unwrap()
            .get_node_as::<AsyncSingleton>(AsyncSingleton::SINGLETON);
        async_singleton.bind().hello();
    }

    fn physics_process(&mut self, delta: f64) {
        // GDScript code:
        //
        // rotation += angular_speed * delta
        // var velocity = Vector2.UP.rotated(rotation) * speed
        // position += velocity * delta

        let radians = (self.angular_speed * delta) as f32;
        self.base_mut().rotate(radians);

        let rotation = self.base().get_rotation();
        let velocity = Vector2::UP.rotated(rotation) * self.speed as f32;
        self.base_mut().translate(velocity * delta as f32);

        // or verbose:
        // let this = self.base_mut();
        // this.set_position(
        //     this.position() + velocity * delta as f32
        // );
    }
}
