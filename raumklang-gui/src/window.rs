struct HannWindow {
    data: Vec<f32>,
}

impl HannWindow {
    pub fn new(width: usize) -> Self {
        let data = (0..width)
            .enumerate()
            .map(|(n, _)| f32::sin((std::f32::consts::PI * n as f32) / width as f32).powi(2))
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

enum Window {
    Hann,
    Tukey(f32),
}

impl Window {
    const ALL: [Window; 2] = [Window::Hann, Window::Tukey(0.0)];
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

struct WindowBuilder {
    left_side: Window,
    left_side_width: usize,
    right_side: Window,
    right_side_width: usize,
    width: usize,
}

impl WindowBuilder {
    pub fn new(left_side: Window, right_side: Window, width: usize) -> Self {
        Self {
            left_side,
            left_side_width: width / 2,
            right_side,
            right_side_width: width / 2,
            width,
        }
    }

    pub fn build(&self) -> Vec<f32> {
        let left = create_window(&self.left_side, self.left_side_width * 2);
        let right = create_window(&self.right_side, self.right_side_width * 2);

        let mut left: Vec<_> = left.into_iter().take(self.left_side_width).collect();
        let mut right: Vec<_> = right.into_iter().skip(self.right_side_width).collect();

        let mut window = Vec::with_capacity(self.width);
        window.append(&mut left);
        window.append(&mut vec![
            1.0;
            self.width
                - self.left_side_width
                - self.right_side_width
        ]);
        window.append(&mut right);

        window
    }

    pub fn set_left_side_width(&mut self, width: usize) {
        self.left_side_width = width;
    }

    pub fn set_right_side_width(&mut self, width: usize) {
        self.right_side_width = width;
    }

    pub fn get_left_side_width(&self) -> usize {
        self.left_side_width
    }

    pub fn get_right_side_width(&self) -> usize {
        self.right_side_width
    }
}

fn create_window(window_type: &Window, width: usize) -> Vec<f32> {
    match window_type {
        Window::Hann => HannWindow::new(width).data,
        Window::Tukey(a) => TukeyWindow::new(width, *a).data,
    }
}
