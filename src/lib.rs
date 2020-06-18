#[macro_use]
extern crate gdnative;

mod godot_toml;

fn init(handle: gdnative::init::InitHandle) {
	handle.add_class::<godot_toml::GodotToml>();
}

godot_gdnative_init!();
godot_nativescript_init!(init);
godot_gdnative_terminate!();