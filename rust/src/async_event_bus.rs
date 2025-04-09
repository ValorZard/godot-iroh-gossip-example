use godot::{classes::Engine, prelude::*};

use crate::async_runtime::AsyncRuntime;

type SendData = i32;

#[derive(GodotClass)]
#[class(base=Object)]
pub struct AsyncEventBus {
    base: Base<Object>,
    receiver: Option<tokio::sync::mpsc::Receiver<SendData>>,
}

#[godot_api]
impl IObject for AsyncEventBus {
    fn init(base: Base<Object>) -> Self {
        Self {
            base,
            receiver: None,
        }
    }

    /*
    fn process(&mut self, delta: f64) {
        if let Some(receiver) = &mut self.receiver {
            while let Ok(value) = receiver.try_recv() {
                godot_print!("Received value: {}", value);
            }
        }
    }
    */
}

#[godot_api]
impl AsyncEventBus {
    pub const SINGLETON: &'static str = "AsyncEventBus";

    /// This function has no real use for the user, only to make it easier
    /// for this crate to access the singleton object.
    pub fn singleton() -> Option<Gd<AsyncEventBus>> {
        match Engine::singleton().get_singleton(Self::SINGLETON) {
            Some(singleton) => Some(singleton.cast::<Self>()),
            None => None,
        }
    }

    pub fn hello_world() {
        godot_print!("Hello from AsyncEventBus!");
    }

    #[func]
    pub fn start_async(&mut self) {
        let runtime = AsyncRuntime::runtime();
        let (sender, receiver) = tokio::sync::mpsc::channel::<SendData>(32);
        runtime.spawn(async move {
            // Your async code here
            let mut i = 0;
            loop {
                sender.send(i).await.unwrap();
                i += 1;
                tokio::time::sleep(tokio::time::Duration::from_millis(1)).await;
            }
        });

        self.receiver = Some(receiver);
        godot_print!("Async event bus started!");
    }

    #[func]
    pub fn poll_receiver(&mut self) -> Array<SendData>{
        let mut array = Array::new();
        if let Some(receiver) = &mut self.receiver {
            while let Ok(value) = receiver.try_recv() {
                //godot_print!("Received value: {}", value);
                array.push(value);
            }
        } else {
            godot_print!("Receiver is not initialized!");
        }
        array
    }
}