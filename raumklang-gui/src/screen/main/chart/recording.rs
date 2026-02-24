use iced::{
    Element, Length::Fill, Point, Rectangle, Renderer, Size, Theme, Vector, mouse, widget::canvas,
};

use crate::{
    data::SampleRate,
    screen::main::chart::{HorizontalAxis, VerticalAxis, Zoom, db_full_scale, time_scale},
};

pub fn record_waveform<'a>(
    sample_rate: SampleRate,
    signal: &'a [f32],
    cache: &'a canvas::Cache,
) -> Element<'a, (), iced::Theme> {
    canvas::Canvas::new(Recording {
        datapoints: signal.iter().copied().enumerate(),
        y_to_float: |s| s,
        to_x_scale: move |i| time_scale(i, sample_rate.into()),
        to_y_scale: move |s| db_full_scale(s),
        zoom: Zoom::default(),
        offset: 0,
        cache,
    })
    .width(Fill)
    .height(Fill)
    .into()
}

struct Recording<'a, I, X, Y, ScaleX, ScaleY>
where
    I: Iterator<Item = (X, Y)>,
{
    datapoints: I,
    to_x_scale: ScaleX,
    y_to_float: fn(Y) -> f32,
    to_y_scale: ScaleY,
    zoom: super::Zoom,
    offset: i64,
    cache: &'a canvas::Cache,
}

impl<'a, I, X, Y, ScaleX, ScaleY> canvas::Program<(), iced::Theme>
    for Recording<'a, I, X, Y, ScaleX, ScaleY>
where
    I: Iterator<Item = (X, Y)> + Clone + 'a,
    Y: Copy + std::iter::Sum,
    ScaleX: Fn(f32) -> f32,
    ScaleY: Fn(f32) -> f32,
{
    type State = ();
    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let palette = theme.extended_palette();

        let x_min = self.offset as f32;

        let x_max = self.datapoints.clone().count() as f32 * f32::from(self.zoom);
        let x_max = x_max.ceil() as u64;

        // TODO make configureable
        let min_value = -90.0;
        let max_value = 6.0;

        let x_range = x_min..=x_max as f32;
        let x_axis = HorizontalAxis::new(x_range, &self.to_x_scale, 10);

        let y_range = min_value..=max_value;
        let y_axis = VerticalAxis::new(y_range, 10);

        let plane = Rectangle::new(
            Point::new(bounds.x + y_axis.width, bounds.y),
            Size::new(bounds.width - y_axis.width, bounds.height - x_axis.height),
        );

        let pixels_per_unit_x = plane.width / x_axis.length;
        let window_size = if pixels_per_unit_x < 1.0 {
            Some((x_axis.length / plane.width).floor() as usize)
        } else {
            None
        };

        let bar_width = if window_size.is_some() {
            1.0
        } else {
            pixels_per_unit_x
        };

        let y_target_length = plane.height - y_axis.min_label_height * 0.5;
        let pixels_per_unit = y_target_length / y_axis.length;

        let data = self.cache.draw(renderer, bounds.size(), |frame| {
            let datapoints = self
                .datapoints
                .clone()
                .skip(if x_min > 0.0 {
                    x_min.ceil() as usize
                } else {
                    0
                })
                .take(x_max.saturating_add_signed(self.offset) as usize)
                .map(|(_i, datapoint)| datapoint);

            let x_min = -x_axis.min;
            for (i, datapoint) in datapoints.enumerate() {
                let value = datapoint;

                let value = (self.y_to_float)(value);
                let value = (self.to_y_scale)(value);

                let bar_height = (value - y_axis.min) * pixels_per_unit;

                let bar = Rectangle {
                    x: y_axis.width + (x_min * pixels_per_unit_x) + (i as f32 * pixels_per_unit_x),
                    y: plane.height - bar_height,
                    width: bar_width,
                    height: bar_height,
                };

                frame.fill_rectangle(bar.position(), bar.size(), palette.secondary.weak.color);
            }

            frame.with_save(|frame| {
                frame.translate(Vector::new(y_axis.width, 0.0));

                x_axis.draw(frame, plane.width);
            });

            y_axis.draw(frame, y_target_length);
        });

        vec![data]
    }
}
