#[derive(Debug, Default, Clone)]
pub enum State {
    #[default]
    NotComputed,
    Computing,
    Computed(raumklang_core::ImpulseResponse),
}
