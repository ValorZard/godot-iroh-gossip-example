use async_event_bus::AsyncEventBus;
use async_runtime::AsyncRuntime;
use godot::{classes::Engine, prelude::*};
mod async_runtime;
mod async_event_bus;
mod player;

struct MyExtension;

#[gdextension]
unsafe impl ExtensionLibrary for MyExtension {
    fn on_level_init(level: InitLevel) {
        match level {
            InitLevel::Scene => {
                let mut engine = Engine::singleton();

                // This is where we register our async runtime singleton.
                engine.register_singleton(AsyncRuntime::SINGLETON, &AsyncRuntime::new_alloc());
                engine.register_singleton(AsyncEventBus::SINGLETON, &AsyncEventBus::new_alloc());
            }
            _ => (),
        }
    }

    fn on_level_deinit(level: InitLevel) {
        match level {
            InitLevel::Scene => {
                let mut engine = Engine::singleton();

                // Here is where we free our async runtime singleton from memory.
                if let Some(async_singleton) = engine.get_singleton(AsyncRuntime::SINGLETON) {
                    engine.unregister_singleton(AsyncRuntime::SINGLETON);
                    async_singleton.free();
                } else {
                    godot_warn!(
                        "Failed to find & free singleton -> {}",
                        AsyncRuntime::SINGLETON
                    );
                }

                // Here is where we free our async runtime singleton from memory.
                if let Some(singleton) = engine.get_singleton(AsyncEventBus::SINGLETON) {
                    engine.unregister_singleton(AsyncEventBus::SINGLETON);
                    singleton.free();
                } else {
                    godot_warn!(
                        "Failed to find & free singleton -> {}",
                        AsyncEventBus::SINGLETON
                    );
                }
            }
            _ => (),
        }
    }
}
