use crate::data::{SampleRate, smooth_fractional_octave};
use crate::widget::sidebar;
use crate::{data, icon};

use iced::Alignment;
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

    pub state: State,
}

#[derive(Debug, Clone)]
pub enum State {
    None,
    WaitingForImpulseResponse,
    Computing,
    Computed(Data),
}

#[derive(Debug, Clone)]
pub struct Data {
    pub origin: data::FrequencyResponse,
    base_smoothed: SpectrumLayer,
    pub smoothed: Option<SpectrumLayer>,
}

#[derive(Debug, Clone)]
pub struct SpectrumLayer(pub Vec<PlotPoint<f32>>);

impl FrequencyResponse {
    pub fn new() -> Self {
        let color = random_color();

        Self {
            color,
            is_shown: true,

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

    pub fn result(&self) -> Option<&Data> {
        let State::Computed(ref data) = self.state else {
            return None;
        };

        Some(data)
    }

    pub fn result_mut(&mut self) -> Option<&mut Data> {
        let State::Computed(data) = &mut self.state else {
            return None;
        };

        Some(data)
    }

    pub fn set_result(&mut self, fr: data::FrequencyResponse) {
        let data = smooth_fractional_octave(&fr.data, 48);

        let sample_rate = fr.sample_rate;
        let len = fr.data.len() * 2 + 1;
        let resolution = sample_rate as f32 / len as f32;

        // FIXME add start and end
        let mut curve = Vec::with_capacity(len);
        for (i, s) in data.iter().enumerate() {
            curve.push(PlotPoint::new(i as f32 * resolution, dbfs(*s)));
        }

        self.state = State::Computed(Data {
            origin: fr,
            base_smoothed: SpectrumLayer(curve),
            smoothed: None,
        })
    }

    pub fn reset_smoothing(&mut self) {
        let State::Computed(data) = &mut self.state else {
            return;
        };

        data.smoothed = None;
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

impl SpectrumLayer {
    pub fn new<I>(data: I, sample_rate: SampleRate) -> Self
    where
        I: IntoIterator<Item = f32>,
        I::IntoIter: Clone,
    {
        let data = data.into_iter();

        let len = data.clone().count() * 2 + 1;
        let resolution = f32::from(sample_rate) / len as f32;

        let curve = data
            .enumerate()
            .map(|(i, s)| PlotPoint::new(i as f32 * resolution, dbfs(s)))
            .collect();

        Self(curve)
    }
}

const MIN_FREQ: f32 = 15.0;
const MAX_FREQ: f32 = 22_000.0;
const MIN_DB: f32 = -90.0;

impl PlotData<f32> for FrequencyResponse {
    fn draw(&self, plot: &mut Plot<f32>, _theme: &iced::Theme) {
        let State::Computed(ref fr) = self.state else {
            return;
        };

        if fr.base_smoothed.0.len() < 2 {
            return;
        }

        // FIXME: area is drawn wrong when moving / zooming in
        let mut fill_points = Vec::with_capacity(fr.base_smoothed.0.len() + 2);
        fill_points.push(PlotPoint::new(MIN_FREQ, MIN_DB));
        fill_points.extend(fr.base_smoothed.0.iter().copied());
        fill_points.push(PlotPoint::new(MAX_FREQ, MIN_DB));

        plot.add_shape(shape::Area::new(fill_points).fill(self.color.scale_alpha(0.1)));

        let line_stroke = Stroke::new(self.color.scale_alpha(0.8), Measure::Screen(1.0));
        if let Some(smoothed) = fr.smoothed.as_ref() {
            plot.add_shape(shape::Polyline::new(smoothed.0.clone()).stroke(line_stroke));
        } else {
            plot.add_shape(shape::Polyline::new(fr.base_smoothed.0.clone()).stroke(line_stroke));
        }
    }
}
