use godot::{classes::{EditorPlugin, IEditorPlugin}, prelude::*};

use crate::async_singleton::AsyncSingleton;

#[derive(GodotClass)]
#[class(tool, init, base=EditorPlugin)]
struct BootstrapEditorPlugin {
    base: Base<EditorPlugin>,
}

#[godot_api]
impl IEditorPlugin for BootstrapEditorPlugin {
    fn enter_tree(&mut self) {
        // Perform typical plugin operations here.

        // we have to do this hack because theres no real good way to add an autoload singleton
        // we HAVE to create a scene that only contains the node we want to autoload as a singleton
        // this also means we have to have a different name for the autoload singleton vs the node itself
        self.base_mut().add_autoload_singleton(AsyncSingleton::SINGLETON, "res://async_event_bus.tscn");
    }

    fn exit_tree(&mut self) {
        // Perform typical plugin operations here.
    }
}
