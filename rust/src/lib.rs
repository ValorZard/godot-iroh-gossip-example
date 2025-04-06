use godot::prelude::*;
mod player;

struct MyExtension;

#[gdextension]
unsafe impl ExtensionLibrary for MyExtension {}
