use std::ops::{AddAssign, SubAssign};

#[derive(Debug, Clone)]
pub struct Handle {
    pub x: f32,
    alignment: Alignment,
}

#[derive(Debug, Clone, Copy)]
pub enum Alignment {
    Bottom,
    Center,
    Top,
}

impl Handle {
    pub fn new(x: f32, alignment: Alignment) -> Self {
        Self { x, alignment }
    }

    pub fn x(&self) -> f32 {
        self.x
    }

    pub fn y(&self) -> Alignment {
        self.alignment
    }
}

impl From<Alignment> for f32 {
    fn from(alignment: Alignment) -> Self {
        Into::into(&alignment)
    }
}

impl From<&Alignment> for f32 {
    fn from(alignment: &Alignment) -> Self {
        match alignment {
            Alignment::Bottom => 0.0,
            Alignment::Center => 0.5,
            Alignment::Top => 1.0,
        }
    }
}
impl SubAssign<f32> for Handle {
    fn sub_assign(&mut self, offset: f32) {
        self.x -= offset;
    }
}

impl AddAssign<f32> for Handle {
    fn add_assign(&mut self, offset: f32) {
        self.x += offset;
    }
}
