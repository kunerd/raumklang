struct HannWindow {
    data: Vec<f32>,
}

impl HannWindow {
    pub fn new(width: usize) -> Self {
        let data = (0..width)
            .map(|n| f32::sin((std::f32::consts::PI * n as f32) / width as f32).powi(2))
            .collect();

        Self { data }
    }
}

struct TukeyWindow {
    data: Vec<f32>,
}

impl TukeyWindow {
    pub fn new(width: usize, alpha: f32) -> Self {
        let lower_bound = (alpha * width as f32 / 2.0) as usize;
        let upper_bound = width / 2;

        let mut data: Vec<f32> = Vec::with_capacity(width);

        for n in 0..=width {
            let s = if n <= lower_bound {
                let num = 2.0 * std::f32::consts::PI * n as f32;
                let denom = alpha * width as f32;
                0.5 * (1.0 - f32::cos(num / denom))
            } else if lower_bound < n && n <= upper_bound {
                1.0
            } else {
                *data.get(width - n).unwrap()
            };

            data.push(s);
        }

        Self { data }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Window {
    Hann,
    Tukey(f32),
}

impl Window {
    pub const ALL: [Window; 2] = [Window::Hann, Window::Tukey(0.25)];
}

impl std::fmt::Display for Window {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Window::Hann => "Hann",
                Window::Tukey(_) => "Tukey",
            }
        )
    }
}

#[derive(Debug)]
pub struct WindowBuilder {
    pub left_side: Window,
    pub left_side_width: usize,
    pub right_side: Window,
    pub right_side_width: usize,
    pub offset: usize,
}

impl WindowBuilder {
    pub fn new(
        left_side: Window,
        left_side_width: usize,
        right_side: Window,
        right_side_width: usize,
    ) -> Self {
        Self {
            left_side,
            left_side_width,
            right_side,
            right_side_width,
            offset: 0,
        }
    }

    pub fn build(&self) -> Vec<f32> {
        let left_window =
            create_window(&self.left_side, self.left_side_width.saturating_sub(1) * 2);
        let left_half_window = left_window.into_iter().take(self.left_side_width);

        let right_window = create_window(
            &self.right_side,
            (self.right_side_width.saturating_sub(1)) * 2,
        );
        let right_half_window = right_window.into_iter().take(self.right_side_width).rev();

        let offset_window = (0..self.offset).map(|_| 1.0f32);

        let mut window = Vec::with_capacity(self.left_side_width + self.right_side_width);
        window.extend(left_half_window);
        window.extend(offset_window);
        window.extend(right_half_window);

        window
    }

    pub fn set_offset(&mut self, offset_width: usize) -> &mut Self {
        self.offset = offset_width;

        self
    }
}

fn create_window(window_type: &Window, width: usize) -> Vec<f32> {
    match window_type {
        Window::Hann => HannWindow::new(width).data,
        Window::Tukey(a) => TukeyWindow::new(width, *a).data,
    }
}

#[cfg(test)]
mod test {
    use super::{Window, WindowBuilder};

    macro_rules! assert_eq_delta {
        ($a:expr, $b:expr, $d:expr) => {
            let left = ($a - $b).abs();
            assert!(
                left <= $d,
                "assert failed: {} == {}, left {} <= delta {}",
                $a,
                $b,
                left,
                $d
            )
        };
    }

    #[test]
    fn no_window() {
        let left_side_width = 0;
        let right_side_width = 0;

        let builder = WindowBuilder::new(
            Window::Hann,
            left_side_width,
            Window::Hann,
            right_side_width,
        );

        let window = builder.build();
        let len = window.len();

        assert_eq!(0, len);
    }

    #[test]
    fn left_side_window_only() {
        let left_side_width = 100;
        let right_side_width = 0;

        let builder = WindowBuilder::new(
            Window::Hann,
            left_side_width,
            Window::Hann,
            right_side_width,
        );
        let window = builder.build();

        assert_eq_delta!(window.first().unwrap(), 0.0, f32::EPSILON);
        assert_eq_delta!(window.last().unwrap(), 1.0, f32::EPSILON);
        assert_eq!(left_side_width, window.len());
    }

    #[test]
    fn right_side_window_only() {
        let left_side_width = 0;
        let right_side_width = 50;

        let builder = WindowBuilder::new(
            Window::Hann,
            left_side_width,
            Window::Hann,
            right_side_width,
        );
        let window = builder.build();

        assert_eq_delta!(window.first().unwrap(), 1.0, f32::EPSILON);
        assert_eq_delta!(window.last().unwrap(), 0.0, f32::EPSILON);
        assert_eq!(right_side_width, window.len());
    }

    #[test]
    fn left_and_right_side() {
        let left_side_width = 50;
        let right_side_width = 50;

        let builder = WindowBuilder::new(
            Window::Hann,
            left_side_width,
            Window::Hann,
            right_side_width,
        );

        let window = builder.build();
        assert_eq_delta!(window[0], 0.0, f32::EPSILON);
        assert_eq_delta!(window[left_side_width], 1.0, f32::EPSILON);
        assert_eq_delta!(window[right_side_width], 1.0, f32::EPSILON);
        assert_eq_delta!(window.last().unwrap(), 0.0, f32::EPSILON);

        let len = window.len();
        assert_eq!(len, left_side_width + right_side_width);
    }

    #[test]
    fn offset_only() {
        let left_side_width = 0;
        let right_side_width = 0;
        let offset_width = 50;

        let mut builder = WindowBuilder::new(
            Window::Hann,
            left_side_width,
            Window::Hann,
            right_side_width,
        );
        builder.set_offset(offset_width);

        let window = builder.build();
        assert_eq_delta!(window[0], 1.0, f32::EPSILON);
        assert_eq_delta!(window[offset_width / 2], 1.0, f32::EPSILON);
        assert_eq_delta!(window.last().unwrap(), 1.0, f32::EPSILON);

        let len = window.len();
        assert_eq!(len, offset_width);
    }

    #[test]
    fn full_window() {
        let left_side_width = 50;
        let right_side_width = 50;
        let offset_width = 50;

        let mut builder = WindowBuilder::new(
            Window::Hann,
            left_side_width,
            Window::Hann,
            right_side_width,
        );
        builder.set_offset(offset_width);

        let window = builder.build();
        assert_eq_delta!(window[0], 0.0, f32::EPSILON);
        assert_eq_delta!(window[left_side_width], 1.0, f32::EPSILON);
        assert_eq_delta!(window[window.len() / 2], 1.0, f32::EPSILON);
        assert_eq_delta!(window[right_side_width], 1.0, f32::EPSILON);
        assert_eq_delta!(window.last().unwrap(), 0.0, f32::EPSILON);

        let len = window.len();
        assert_eq!(len, left_side_width + offset_width + right_side_width);
    }
}
