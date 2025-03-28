pub mod handle;

pub use handle::Handle;

use std::time::Duration;

#[derive(Debug, Clone, Copy)]
pub struct Samples(usize);

impl From<Samples> for f32 {
    fn from(samples: Samples) -> Self {
        samples.0 as f32
    }
}

// pub enum TimeScale{
//     Samples(usize),
//     Duration{
//         duration: Duration,
//         sample_rate: u32
//     }
// }

impl Samples {
    pub fn from_duration(duration: Duration, sample_rate: u32) -> Self {
        let samples = duration.as_secs_f32() * sample_rate as f32;

        Self(samples.round() as usize)
    }
}

#[derive(Debug, Clone)]
pub struct Window<D> {
    left_type: raumklang_core::Window,
    left_width: D,
    position: D,
    right_type: raumklang_core::Window,
    right_width: D,
}

// #[derive(Debug, Clone, Copy, PartialEq)]
// pub enum Type {
//     Hann,
//     Tukey(f32),
// }

pub struct Handles {
    left: Handle,
    center: Handle,
    right: Handle,
}

impl Default for Window<Duration> {
    fn default() -> Self {
        Self {
            left_type: raumklang_core::Window::Tukey(0.25),
            left_width: Duration::from_millis(125),
            position: Duration::from_millis(0),
            right_type: raumklang_core::Window::Tukey(0.25),
            right_width: Duration::from_millis(500),
        }
    }
}

impl Window<Samples> {
    pub fn from_duration(window: Window<Duration>, sample_rate: u32) -> Self {
        Self {
            left_type: window.left_type,
            left_width: Samples::from_duration(window.left_width, sample_rate),
            position: Samples::from_duration(window.position, sample_rate),
            right_type: window.right_type,
            right_width: Samples::from_duration(window.right_width, sample_rate),
        }
    }

    pub fn curve(&self) -> impl Iterator<Item = (f32, f32)> + Clone + use<'_> {
        let builder = raumklang_core::WindowBuilder::new(
            self.left_type,
            self.left_width.0,
            self.right_type,
            self.right_width.0,
        );

        builder.build().into_iter().enumerate().map(|(i, s)| {
            let left_width: f32 = self.left_width.into();
            let x = i as f32 - left_width;

            (x, s)
        })
    }
}

// impl Type {
//     pub const ALL: [Self; 2] = [Self::Hann, Self::Tukey(0.25)];
// }

// impl Default for Type {
//     fn default() -> Self {
//         Self::Tukey(0.25)
//     }
// }

// impl std::fmt::Display for Type {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//         write!(
//             f,
//             "{}",
//             match self {
//                 Type::Hann => "Hann",
//                 Type::Tukey(_) => "Tukey",
//             }
//         )
//     }
// }

impl Handles {
    pub fn iter(&self) -> std::array::IntoIter<&Handle, 3> {
        [&self.left, &self.center, &self.right].into_iter()
    }
}

// impl<'a> IntoIterator for &'a Handles {
//     type Item = &'a Handle;
//     type IntoIter = std::array::IntoIter<Self::Item, 3>;

//     fn into_iter(self) -> Self::IntoIter {
//         self.iter()
//     }
// }

impl From<&Window<Samples>> for Handles {
    fn from(window: &Window<Samples>) -> Self {
        let position: f32 = window.position.into();
        let left_width: f32 = window.left_width.into();

        let start_pos: f32 = position - left_width;
        let left = Handle::new(start_pos, handle::Alignment::Bottom);

        let center = Handle::new(position, handle::Alignment::Top);

        let right_width: f32 = window.right_width.into();
        let right_pos = position + right_width;
        let right = Handle::new(right_pos, handle::Alignment::Bottom);

        Self {
            left,
            center,
            right,
        }
    }
}
