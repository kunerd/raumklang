pub mod handle;

pub use handle::Handle;

use super::{SampleRate, Samples};

use std::time::Duration;

#[derive(Debug, Clone, PartialEq)]
pub struct Window<D = Samples> {
    sample_rate: SampleRate,
    left_type: raumklang_core::Window,
    left_width: D,
    position: D,
    right_type: raumklang_core::Window,
    right_width: D,
}

#[derive(Debug)]
pub struct Handles {
    left: Handle,
    center: Handle,
    right: Handle,
}

impl<D> Window<D> {
    pub fn sample_rate(&self) -> SampleRate {
        self.sample_rate
    }
}

impl Window<Duration> {
    pub fn new(sample_rate: SampleRate) -> Self {
        Self {
            sample_rate,
            left_type: raumklang_core::Window::Tukey(0.25),
            left_width: Duration::from_millis(125),
            position: Duration::from_millis(0),
            right_type: raumklang_core::Window::Tukey(0.25),
            right_width: Duration::from_millis(500),
        }
    }

    pub fn update(&mut self, handles: Handles) {
        let left_width = handles.center.x() - handles.left.x();
        self.left_width = Duration::from_millis(left_width as u64);

        self.position = Duration::from_millis(handles.center.x() as u64);

        let right_width = handles.right.x() - handles.center.x();
        self.right_width = Duration::from_millis(right_width as u64);
    }
}

impl From<Window<Duration>> for Window<Samples> {
    fn from(window: Window<Duration>) -> Self {
        let sample_rate = window.sample_rate;

        Self {
            sample_rate,
            left_type: window.left_type,
            left_width: Samples::from_duration(window.left_width, sample_rate),
            position: Samples::from_duration(window.position, sample_rate),
            right_type: window.right_type,
            right_width: Samples::from_duration(window.right_width, sample_rate),
        }
    }
}

impl Window<Samples> {
    pub fn curve(&self) -> impl Iterator<Item = (f32, f32)> + Clone + use<'_> {
        let builder = raumklang_core::WindowBuilder::new(
            self.left_type,
            self.left_width.into(),
            self.right_type,
            self.right_width.into(),
        );

        builder.build().into_iter().enumerate().map(|(i, s)| {
            let position: f32 = self.position.into();
            let left_width: f32 = self.left_width.into();
            let x = i as f32 + position - left_width;

            (x, s)
            // (i as f32, s)
        })
    }

    pub fn update(&mut self, handles: Handles) {
        let left_width = handles.center.x() - handles.left.x();
        self.left_width = Samples::from_f32(left_width, self.sample_rate);

        self.position = Samples::from_f32(handles.center.x(), self.sample_rate);

        self.right_width =
            Samples::from_f32(handles.right.x() - handles.center.x(), self.sample_rate);
    }

    pub fn offset(&self) -> Samples {
        self.left_width - self.position
    }
}

impl From<Window<Samples>> for Window<Duration> {
    fn from(window: Window<Samples>) -> Self {
        let sample_rate = window.sample_rate;
        Self {
            sample_rate,
            left_type: window.left_type,
            left_width: Duration::from(window.left_width),
            position: Duration::from(window.position),
            right_type: window.right_type,
            right_width: Duration::from(window.right_width),
        }
    }
}

impl Handles {
    pub fn get(&self, id: usize) -> &Handle {
        match id {
            0 => &self.left,
            1 => &self.center,
            2 => &self.right,
            _ => panic!("not a valid ID"),
        }
    }

    pub fn iter(&self) -> std::array::IntoIter<&Handle, 3> {
        [&self.left, &self.center, &self.right].into_iter()
    }

    pub fn move_left(&mut self, offset: f32) {
        self.left += offset;
    }

    pub fn move_center(&mut self, offset: f32) {
        self.left += offset;
        self.center += offset;
        self.right += offset;
    }

    pub fn move_right(&mut self, offset: f32) {
        self.right += offset;
    }

    pub fn update(&mut self, index: usize, new_pos: f32) {
        match index {
            0 => self.left.x = new_pos,
            1 => {
                self.left.x = new_pos - (self.center.x - self.left.x);
                self.right.x = new_pos + (self.right.x - self.center.x);
                self.center.x = new_pos;
            }
            2 => self.right.x = new_pos,
            n => panic!("there should be no handles with index: {n}"),
        };
    }
}

impl IntoIterator for Handles {
    type Item = Handle;
    type IntoIter = std::array::IntoIter<Self::Item, 3>;

    fn into_iter(self) -> Self::IntoIter {
        [self.left, self.center, self.right].into_iter()
    }
}

impl<'a> IntoIterator for &'a Handles {
    type Item = &'a Handle;
    type IntoIter = std::array::IntoIter<Self::Item, 3>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

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

impl From<&Window<Duration>> for Handles {
    fn from(window: &Window<Duration>) -> Self {
        let position: f32 = window.position.as_millis() as f32;
        let left_width: f32 = window.left_width.as_millis() as f32;

        let start_pos: f32 = position - left_width;
        let left = Handle::new(start_pos, handle::Alignment::Bottom);

        let center = Handle::new(position, handle::Alignment::Top);

        let right_width: f32 = window.right_width.as_millis() as f32;
        let right_pos = position + right_width;
        let right = Handle::new(right_pos, handle::Alignment::Bottom);

        Self {
            left,
            center,
            right,
        }
    }
}
