use iced::{
    Font, Pixels, Point, Renderer, Size,
    advanced::{graphics::text::Paragraph, text::Paragraph as _},
    widget::{
        canvas,
        text::{LineHeight, Shaping},
    },
};

pub struct RmsPeakMeter<'a> {
    rms: f32,
    peak: f32,
    ticks: Vec<i8>,
    state: State,
    cache: &'a canvas::Cache,
}

pub enum State {
    Normal,
    Warning,
    Danger,
}

impl<'a> RmsPeakMeter<'a> {
    pub fn new(rms: f32, peak: f32, cache: &'a canvas::Cache) -> Self {
        let ticks = vec![6, 0, -6, -12, -24, -48, -70];
        Self {
            rms,
            peak,
            ticks,
            state: State::Normal,
            cache,
        }
    }

    pub fn state(mut self, state: State) -> Self {
        self.state = state;
        self
    }
}

impl<'a, Message> canvas::Program<Message> for RmsPeakMeter<'a> {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        theme: &iced::Theme,
        bounds: iced::Rectangle,
        _cursor: iced::advanced::mouse::Cursor,
    ) -> Vec<canvas::Geometry<Renderer>> {
        let font_size = Pixels::from(10);

        let min = self.ticks.iter().min().copied().unwrap_or(-70) as f32;
        let max = self.ticks.iter().max().copied().unwrap_or(6) as f32;

        let tick_thickness = 1.0;
        let tick_length = 6.0;
        let tick_label_space = 4.0;

        let max_text_bounds = Paragraph::with_text(iced::advanced::Text {
            content: &format!("{:.0}", min),
            size: font_size,
            font: Font::MONOSPACE,
            align_x: iced::widget::text::Alignment::Right,
            align_y: iced::alignment::Vertical::Center,
            bounds: Size::INFINITE,
            line_height: LineHeight::default(),
            shaping: iced::widget::text::Shaping::Basic,
            wrapping: iced::widget::text::Wrapping::None,
        })
        .min_bounds();

        let height = bounds.height - max_text_bounds.height;
        let width = bounds.width - max_text_bounds.width - tick_length - tick_label_space;

        let border_width = 2.0;

        let meter = self.cache.draw(renderer, bounds.size(), |frame| {
            let offset = max_text_bounds.height / 2.0;
            let palette = theme.extended_palette();

            frame.fill_rectangle(
                Point::new(0.0, offset),
                Size::new(width, height),
                palette.background.weak.color,
            );

            let range = min - max;
            let pixel_per_unit = height / range;

            let color = match self.state {
                State::Normal => palette.success.base.color,
                State::Warning => palette.warning.base.color,
                State::Danger => palette.danger.base.color,
            };
            // .scale_alpha(0.6);

            let rms_heigh = pixel_per_unit * self.rms.clamp(min, max) - max * pixel_per_unit;
            frame.fill_rectangle(
                Point::new(border_width, rms_heigh + offset),
                Size::new(
                    width - 2.0 * border_width,
                    height - rms_heigh - border_width,
                ),
                color,
            );

            let peak_heigh = pixel_per_unit * self.peak.clamp(min, max) - max * pixel_per_unit;
            frame.fill_rectangle(
                Point::new(
                    border_width,
                    peak_heigh.clamp(border_width, rms_heigh) + offset,
                ),
                Size::new(width - 2.0 * border_width, border_width),
                palette.secondary.strong.color,
            );

            for n in &self.ticks {
                let y = *n as f32 * pixel_per_unit - max * pixel_per_unit + offset;
                frame.fill_rectangle(
                    Point::new(width + 2.0, y),
                    Size::new(tick_length, tick_thickness),
                    palette.secondary.strong.color,
                );

                frame.fill_text(canvas::Text {
                    content: format!("{:.0}", n),
                    position: Point::new(
                        width + tick_length + tick_label_space + max_text_bounds.width,
                        y,
                    ),
                    color: palette.background.base.text,
                    size: font_size,
                    font: Font::MONOSPACE,
                    align_x: iced::widget::text::Alignment::Right,
                    align_y: iced::alignment::Vertical::Center,
                    max_width: f32::INFINITY,
                    line_height: LineHeight::default(),
                    shaping: Shaping::Basic,
                });
            }
        });

        vec![meter]
    }
}
