use std::time::Duration;

#[derive(Debug)]
pub struct Window {
    start: Duration,
    left_type: Type,
    left_width: Duration,
    offset: Duration,
    right_type: Type,
    right_width: Duration,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Type {
    Hann,
    Tukey(f32),
}

impl Default for Type {
    fn default() -> Self {
        Self::Tukey(0.25)
    }
}

impl Default for Window {
    fn default() -> Self {
        Self {
            start: Duration::from_millis(0),
            left_type: Type::default(),
            left_width: Duration::from_millis(125),
            offset: Duration::from_millis(0),
            right_type: Type::default(),
            right_width: Duration::from_millis(500),
        }
    }
}

impl Type {
    pub const ALL: [Self; 2] = [Self::Hann, Self::Tukey(0.25)];
}

impl std::fmt::Display for Type {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Type::Hann => "Hann",
                Type::Tukey(_) => "Tukey",
            }
        )
    }
}
