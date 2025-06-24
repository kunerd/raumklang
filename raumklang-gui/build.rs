pub fn main() {
    println!("cargo::rerun-if-changed=fonts/icons.toml");
    iced_fontello::build("fonts/icons.toml").expect("Build icons font");
}
