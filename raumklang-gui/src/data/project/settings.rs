#[derive(Debug, Clone, Default)]
pub struct Settings {
    pub record_to: Target,
}

#[derive(Debug, Clone, Default)]
pub enum Target {
    #[default]
    Memory,
    File,
}
