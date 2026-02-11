use crate::data::smooth_fractional_octave;
use crate::widget::sidebar;
use crate::{data, icon};

use iced::Alignment;
use iced::theme::{Base, Mode};
use iced::widget::stack;
use iced::widget::text::IntoFragment;
use iced::{
    Element, Length,
    widget::{column, container, row, text, toggler},
};

use iced_aksel::{Measure, Plot, PlotData, PlotPoint, Stroke, shape};
use rand::Rng as _;
use raumklang_core::dbfs;

#[derive(Debug, Clone)]
pub struct FrequencyResponse {
    pub color: iced::Color,
    pub is_shown: bool,

    pub smoothed: Option<Box<[f32]>>,

    pub state: State,
}

#[derive(Debug, Clone)]
pub enum State {
    None,
    WaitingForImpulseResponse,
    Computing,
    // Computed(FrequencyResponse),
    Computed(SpectrumLayer),
}

#[derive(Debug, Clone)]
pub struct SpectrumLayer(Vec<PlotPoint<f32>>);

impl FrequencyResponse {
    pub fn new() -> Self {
        let color = random_color();

        Self {
            color,
            is_shown: true,
            smoothed: None,

            state: State::None,
        }
    }

    pub fn view<'a, Message>(
        &'a self,
        measurement_name: &'a str,
        on_toggle: impl Fn(bool) -> Message + 'a,
    ) -> Element<'a, Message>
    where
        Message: Clone + 'a,
    {
        let item = {
            let color_dot = icon::record().color(self.color).align_y(Alignment::Center);

            let content = container(
                text(measurement_name)
                    .size(16)
                    .style(|theme| {
                        let mut base = text::default(theme);

                        let palette = theme.extended_palette();
                        base.color = Some(palette.background.weakest.text);

                        base
                    })
                    .wrapping(text::Wrapping::Glyph)
                    .align_y(Alignment::Center),
            )
            .width(Length::Fill)
            .clip(true);

            let switch =
                container(toggler(self.is_shown).on_toggle(on_toggle)).align_right(Length::Shrink);

            row![color_dot, content, switch]
                .align_y(Alignment::Center)
                .spacing(10)
                .padding(20)
                .into()
        };

        let content = match self.state {
            State::None => item,
            State::WaitingForImpulseResponse => processing_overlay("Impulse Response", item),
            State::Computing => processing_overlay("Computing ...", item),
            State::Computed(_) => item,
        };

        sidebar::item(content, false)
    }

    pub fn result(&self) -> Option<&SpectrumLayer> {
        let State::Computed(ref result) = self.state else {
            return None;
        };

        Some(result)
    }

    pub fn set_result(&mut self, fr: data::FrequencyResponse) {
        // let magnitudes = &self.magnitudes;
        // let sample_rate = self.sample_rate as f64;
        // let tilt = self.tilt;

        // let log_min = MIN_FREQ.log10();
        // let log_max = MAX_FREQ.log10();
        // // let octaves = (log_max - log_min) / (2.0_f32).log10();
        // let len = fr.data.len() * 2 + 1;
        // let octaves = fr.sample_rate as f32 / len as f32;
        // let num_points = (octaves * POINTS_PER_OCTAVE as f32).round().max(32.0) as usize;
        // let step = (log_max - log_min) / num_points as f32;

        // let mut curve = Vec::with_capacity(num_points);
        // for i in 0..num_points {
        //     let freq = 10_f32.powf(log_min + step * i as f32);
        //     // let width = math::fractional_width(freq);
        //     // let db = math::sample_fractional_octave(magnitudes, freq, sample_rate, width, tilt);
        //     curve.push(PlotPoint::new(freq, dbfs(fr.data[i])));
        // }

        let data = smooth_fractional_octave(&fr.data, 48);

        let sample_rate = fr.sample_rate;
        let len = fr.data.len() * 2 + 1;
        let resolution = sample_rate as f32 / len as f32;

        let mut curve = Vec::with_capacity(len);
        for (i, s) in data.iter().enumerate() {
            curve.push(PlotPoint::new(i as f32 * resolution, dbfs(*s)));
        }

        self.state = State::Computed(SpectrumLayer(curve))
    }
}

impl Default for FrequencyResponse {
    fn default() -> Self {
        Self::new()
    }
}

fn random_color() -> iced::Color {
    const MAX_COLOR_VALUE: u8 = 255;

    // TODO: replace with color palette
    let red = rand::thread_rng().gen_range(0..MAX_COLOR_VALUE);
    let green = rand::thread_rng().gen_range(0..MAX_COLOR_VALUE);
    let blue = rand::thread_rng().gen_range(0..MAX_COLOR_VALUE);

    iced::Color::from_rgb8(red, green, blue)
}

fn processing_overlay<'a, Message>(
    status: impl IntoFragment<'a>,
    entry: impl Into<Element<'a, Message>>,
) -> Element<'a, Message>
where
    Message: 'a,
{
    stack([
        container(entry).style(container::bordered_box).into(),
        container(column![text("Computing..."), text(status).size(12)])
            .center(Length::Fill)
            .style(|theme| container::Style {
                border: container::rounded_box(theme).border,
                background: Some(iced::Background::Color(iced::Color::from_rgba(
                    0.0, 0.0, 0.0, 0.8,
                ))),
                ..Default::default()
            })
            .into(),
    ])
    .into()
}

const MIN_FREQ: f32 = 15.0;
const MAX_FREQ: f32 = 22_000.0;
const MIN_DB: f32 = -90.0;
const MAX_DB: f32 = 12.0;
const POINTS_PER_OCTAVE: usize = 72;

impl PlotData<f32> for FrequencyResponse {
    fn draw(&self, plot: &mut Plot<f32>, theme: &iced::Theme) {
        let State::Computed(ref fr) = self.state else {
            return;
        };

        if fr.0.len() < 2 {
            return;
        }

        let mut fill_points = Vec::with_capacity(fr.0.len() + 2);
        fill_points.push(PlotPoint::new(MIN_FREQ, MIN_DB));
        fill_points.extend(fr.0.iter().copied());
        fill_points.push(PlotPoint::new(MAX_FREQ, MIN_DB));

        plot.add_shape(shape::Area::new(fill_points).fill(self.color.scale_alpha(0.1)));

        // let glow_color = if theme.mode() == Mode::Light {
        //     palette.primary.strong.color
        // } else {
        //     palette.primary.weak.color
        // };

        // let glow_stroke = Stroke::new(glow_color, Measure::Screen(6.0));
        // plot.add_shape(shape::Polyline::new(fr.0.clone()).stroke(glow_stroke));

        let line_stroke = Stroke::new(self.color.scale_alpha(0.8), Measure::Screen(1.0));
        plot.add_shape(shape::Polyline::new(fr.0.clone()).stroke(line_stroke));
    }
}
