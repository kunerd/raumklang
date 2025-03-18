use std::collections::HashMap;

use iced::{
    widget::{
        button, checkbox, column, container, horizontal_rule, horizontal_space, pick_list, row,
        scrollable, text,
    },
    Alignment, Element,
    Length::{self, FillPortion},
    Task,
};
use pliced::chart::{line_series, point_series, Chart, Labels, PointStyle};
use raumklang_core::WindowBuilder;

use crate::{
    data,
    widgets::charts::{AmplitudeUnit, TimeSeriesUnit},
    OfflineMeasurement,
};

use super::compute_impulse_response;

#[derive(Debug, Clone)]
pub enum Message {
    MeasurementSelected(data::MeasurementId),
    ImpulseResponseComputed((data::MeasurementId, raumklang_core::ImpulseResponse)),
    Chart(ChartOperation),
    Window(WindowOperation),
}

pub enum Event {
    ImpulseResponseComputed(data::MeasurementId, raumklang_core::ImpulseResponse),
}

pub struct ImpulseResponseTab {
    selected: Option<data::MeasurementId>,
    window: Option<Window>,
    chart_data: ChartData,
}

struct Window {
    max_size: f32,
    sample_rate: f32,
    handles: Vec<WindowHandle>,
    dragging: Dragging,
    hovered_item: Option<usize>,
}

struct WindowHandle {
    x: f32,
    y: f32,
    style: PointStyle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum SeriesId {
    PointList,
}

#[derive(Debug, Default)]
enum Dragging {
    CouldStillBeClick(usize, iced::Point),
    ForSure(usize, iced::Point),
    #[default]
    None,
}

#[derive(Default)]
pub struct ChartData {
    show_window: bool,
    amplitude_unit: AmplitudeUnit,
    time_unit: TimeSeriesUnit,
}

impl ChartData {
    fn apply(&mut self, operation: ChartOperation) {
        match operation {
            ChartOperation::TimeUnitChanged(time_unit) => self.time_unit = time_unit,
            ChartOperation::AmplitudeUnitChanged(amplitude_unit) => {
                self.amplitude_unit = amplitude_unit
            }
            ChartOperation::ShowWindowToggled(state) => self.show_window = state,
        }
    }
}

#[derive(Debug, Clone)]
pub enum ChartOperation {
    TimeUnitChanged(TimeSeriesUnit),
    AmplitudeUnitChanged(AmplitudeUnit),
    ShowWindowToggled(bool),
}

#[derive(Debug, Clone)]
pub enum WindowOperation {
    OnMove(Option<usize>, Option<iced::Point>),
    MouseDown(Option<usize>, Option<iced::Point>),
    MouseUp(Option<iced::Point>),
}

impl Window {
    const DEFAULT_LEFT_DURATION: f32 = 125.0; //ms
    const DEFAULT_RIGTH_DURATION: f32 = 500.0; //ms

    pub fn new(sample_rate: u32, max_size: usize) -> Self {
        let max_size = max_size as f32;
        let half_size = max_size as f32 / 2.0;

        let sample_rate = sample_rate as f32;
        let left_window_size =
            (sample_rate * (Self::DEFAULT_LEFT_DURATION / 1000.0)).min(half_size);
        let right_window_size =
            (sample_rate * (Self::DEFAULT_RIGTH_DURATION / 1000.0)).min(half_size);

        let left_side_left = 0.0;
        let left_side_right = left_side_left + left_window_size as f32;
        let right_side_left = left_side_right;
        let right_side_right = right_side_left + right_window_size as f32;

        let handles = vec![
            WindowHandle::new(left_side_left, 0.0),
            WindowHandle::new(left_side_right, 1.0),
            WindowHandle::new(right_side_left, 1.0),
            WindowHandle::new(right_side_right, 0.0),
        ];

        Self {
            max_size,
            sample_rate,
            handles,
            dragging: Dragging::None,
            hovered_item: None,
        }
    }

    fn apply(&mut self, operation: WindowOperation, time_unit: TimeSeriesUnit) {
        let mut update_handle_pos =
            |id: usize, prev_pos: iced::Point, pos: iced::Point| -> iced::Point {
                let min = match id {
                    0 => f32::MIN,
                    id => self.handles[id - 1].x,
                };

                let max = if let Some(handle) = self.handles.get(id + 1) {
                    handle.x
                } else {
                    self.max_size
                };

                let Some(handle) = self.handles.get_mut(id) else {
                    return prev_pos;
                };

                let offset = prev_pos.x - pos.x;
                let offset = match time_unit {
                    TimeSeriesUnit::Time => offset / 1000.0 * self.sample_rate,
                    TimeSeriesUnit::Samples => offset,
                };

                let new_pos = handle.x - offset;

                if new_pos >= min {
                    if new_pos <= max {
                        handle.x = new_pos;
                        pos
                    } else {
                        let mut x_clamped = handle.x - max;
                        if matches!(time_unit, TimeSeriesUnit::Time) {
                            x_clamped *= 1000.0 / self.sample_rate;
                        }
                        x_clamped = prev_pos.x - x_clamped;

                        handle.x = max;

                        iced::Point::new(x_clamped, pos.y)
                    }
                } else {
                    let mut x_clamped = handle.x - min;
                    if matches!(time_unit, TimeSeriesUnit::Time) {
                        x_clamped *= 1000.0 / self.sample_rate;
                    }
                    x_clamped = prev_pos.x - x_clamped;

                    handle.x = min;

                    iced::Point::new(x_clamped, pos.y)
                }
            };

        match operation {
            WindowOperation::MouseDown(id, pos) => {
                let Dragging::None = self.dragging else {
                    return;
                };

                if let (Some(id), Some(pos)) = (id, pos) {
                    self.dragging = Dragging::CouldStillBeClick(id, pos);
                }
            }
            WindowOperation::OnMove(id, pos) => {
                let Some(pos) = pos else {
                    return;
                };

                match self.dragging {
                    Dragging::CouldStillBeClick(id, prev_pos) => {
                        if prev_pos != pos {
                            let pos = update_handle_pos(id, prev_pos, pos);
                            self.dragging = Dragging::ForSure(id, pos);
                        }
                    }
                    Dragging::ForSure(id, prev_pos) => {
                        let pos = update_handle_pos(id, prev_pos, pos);
                        self.dragging = Dragging::ForSure(id, pos);
                    }
                    Dragging::None => {
                        if id.is_none() {
                            if let Some(handle) =
                                self.hovered_item.and_then(|id| self.handles.get_mut(id))
                            {
                                handle.style = PointStyle::default();
                            }
                        }
                        self.hovered_item = id;
                    }
                }
            }
            WindowOperation::MouseUp(pos) => {
                let Some(pos) = pos else {
                    return;
                };

                match self.dragging {
                    Dragging::CouldStillBeClick(id, _point) => {
                        if let Some(handle) = self.handles.get_mut(id) {
                            handle.style = PointStyle::default();
                        }
                        self.hovered_item = None;
                        self.dragging = Dragging::None;
                    }
                    Dragging::ForSure(id, prev_pos) => {
                        update_handle_pos(id, prev_pos, pos);
                        if let Some(handle) = self.handles.get_mut(id) {
                            handle.style = PointStyle::default();
                        }
                        self.dragging = Dragging::None;
                    }
                    Dragging::None => {}
                }
            }
        }

        let yellow: iced::Color = iced::Color::from_rgb8(238, 230, 0);
        let green: iced::Color = iced::Color::from_rgb8(50, 205, 50);

        match self.dragging {
            Dragging::CouldStillBeClick(id, _point) | Dragging::ForSure(id, _point) => {
                if let Some(handle) = self.handles.get_mut(id) {
                    handle.style = PointStyle {
                        color: Some(green),
                        radius: 10.0,
                        ..Default::default()
                    }
                }
            }
            Dragging::None => {
                if let Some(handle) = self.hovered_item.and_then(|id| self.handles.get_mut(id)) {
                    handle.style = PointStyle {
                        color: Some(yellow),
                        radius: 8.0,
                        ..Default::default()
                    }
                }
            }
        }
    }

    fn curve(&self) -> impl Iterator<Item = (f32, f32)> + Clone {
        let left_side = raumklang_core::Window::Hann;
        let right_side = raumklang_core::Window::Hann;

        let left_side_width = (self.handles[1].x - self.handles[0].x).round() as usize;
        let offset = (self.handles[2].x - self.handles[1].x).round() as usize;
        let right_side_width = (self.handles[3].x - self.handles[2].x).round() as usize;
        let window: Vec<_> =
            WindowBuilder::new(left_side, left_side_width, right_side, right_side_width)
                .set_offset(offset)
                .build()
                .into_iter()
                .enumerate()
                .map(|(x, y)| (x as f32 + self.handles[0].x, y))
                .collect();

        window.into_iter()
    }
}

impl WindowHandle {
    pub fn new(x: f32, y: f32) -> Self {
        Self {
            x,
            y,
            style: PointStyle::default(),
        }
    }
}

impl ImpulseResponseTab {
    pub fn new() -> Self {
        Self {
            selected: None,
            chart_data: ChartData::default(),
            window: None,
        }
    }

    pub fn update(
        &mut self,
        message: Message,
        loopback: &data::Loopback,
        measurements: &data::Store<data::Measurement, OfflineMeasurement>,
        impulse_response: &HashMap<data::MeasurementId, raumklang_core::ImpulseResponse>,
    ) -> (Task<Message>, Option<Event>) {
        match message {
            Message::MeasurementSelected(id) => {
                self.selected = Some(id);

                if let Some(ir) = impulse_response.get(&id) {
                    self.window = Some(Window::new(ir.sample_rate, ir.data.len()));

                    (Task::none(), None)
                } else {
                    let measurement = measurements.get_loaded_by_id(&id);
                    if let Some(measurement) = measurement {
                        (
                            Task::perform(
                                compute_impulse_response(
                                    id,
                                    loopback.0.data.clone(),
                                    measurement.data.clone(),
                                ),
                                Message::ImpulseResponseComputed,
                            ),
                            None,
                        )
                    } else {
                        (Task::none(), None)
                    }
                }
            }
            Message::ImpulseResponseComputed((id, ir)) => {
                self.window = Some(Window::new(ir.sample_rate, ir.data.len()));

                (Task::none(), Some(Event::ImpulseResponseComputed(id, ir)))
            }
            Message::Chart(operation) => {
                self.chart_data.apply(operation);

                (Task::none(), None)
            }
            Message::Window(operation) => {
                if let Some(window) = self.window.as_mut() {
                    window.apply(operation, self.chart_data.time_unit);
                }

                (Task::none(), None)
            }
        }
    }

    pub fn view<'a>(
        &'a self,
        measurements: impl Iterator<Item = (&'a data::MeasurementId, &'a data::Measurement)>,
        impulse_responses: &'a HashMap<data::MeasurementId, raumklang_core::ImpulseResponse>,
    ) -> Element<'a, Message> {
        let list = {
            let entries: Vec<Element<_>> = measurements
                .map(|(i, m)| {
                    let style = if self.selected == Some(*i) {
                        button::primary
                    } else {
                        button::secondary
                    };

                    button(m.name.as_str())
                        .on_press(Message::MeasurementSelected(*i))
                        .style(style)
                        .width(Length::Fill)
                        .into()
                })
                .collect();

            let content = scrollable(column(entries).spacing(5)).into();

            container(list_category("Measurements", content))
                .style(container::rounded_box)
                .height(Length::Fill)
                .padding(8)
        };

        let content = if let Some(impulse_response) = self
            .selected
            .as_ref()
            .and_then(|id| impulse_responses.get(id))
        {
            let chart_menu = row![
                text("Amplitude unit:"),
                pick_list(
                    &AmplitudeUnit::ALL[..],
                    Some(&self.chart_data.amplitude_unit),
                    |unit| Message::Chart(ChartOperation::AmplitudeUnitChanged(unit))
                ),
                text("Time unit:"),
                pick_list(
                    &TimeSeriesUnit::ALL[..],
                    Some(&self.chart_data.time_unit),
                    |unit| { Message::Chart(ChartOperation::TimeUnitChanged(unit)) }
                ),
                checkbox("Show Window", self.chart_data.show_window)
                    .on_toggle(|state| Message::Chart(ChartOperation::ShowWindowToggled(state))),
            ]
            .align_y(Alignment::Center)
            .spacing(10);

            let chart = chart_view(&self.chart_data, impulse_response, self.window.as_ref());
            container(column![chart_menu, chart])
        } else {
            container(text(
                "Please select a measurement to compute the corresponding impulse response.",
            ))
            .center(Length::Fill)
        };

        row![
            list.width(Length::FillPortion(1)),
            content.width(FillPortion(4))
        ]
        .spacing(10)
        .into()
    }
}

fn chart_view<'a>(
    chart_data: &'a ChartData,
    impulse_response: &'a raumklang_core::ImpulseResponse,
    window: Option<&'a Window>,
) -> Element<'a, Message> {
    let max = impulse_response
        .data
        .iter()
        .map(|s| s.re.powi(2).sqrt())
        .fold(f32::NEG_INFINITY, |a, b| a.max(b));

    fn percent_full_scale(s: f32, max: f32) -> f32 {
        s / max * 100f32
    }

    fn db_full_scale(s: f32, max: f32) -> f32 {
        let y = 20f32 * f32::log10(s.abs() / max);
        y.clamp(-100.0, max)
    }

    let y_scale_fn: fn(f32, f32) -> f32 = match chart_data.amplitude_unit {
        AmplitudeUnit::PercentFullScale => percent_full_scale,
        AmplitudeUnit::DezibelFullScale => db_full_scale,
    };

    fn sample_scale(index: f32, _sample_rate: f32) -> f32 {
        index
    }

    fn time_scale(index: f32, sample_rate: f32) -> f32 {
        index / sample_rate * 1000.0
    }

    let x_scale_fn = match chart_data.time_unit {
        TimeSeriesUnit::Samples => sample_scale,
        TimeSeriesUnit::Time => time_scale,
    };

    let sample_rate = impulse_response.sample_rate as f32;
    let series = impulse_response
        .data
        .iter()
        .map(|s| s.re.powi(2).sqrt())
        .enumerate()
        .map(move |(i, s)| (x_scale_fn(i as f32, sample_rate), y_scale_fn(s, max)));

    let chart = Chart::new()
        .width(Length::Fill)
        .height(Length::Fill)
        .x_range(x_scale_fn(-44_10.0, sample_rate)..=x_scale_fn(44_100.0, sample_rate))
        .y_labels(Labels::default().format(&|v| format!("{v:.2}")))
        .push_series(line_series(series).color(iced::Color::from_rgb8(2, 125, 66)));

    if !chart_data.show_window {
        return chart.into();
    }

    let chart = if let Some(window) = window {
        let curve = window.curve();
        chart
            .push_series(
                line_series(
                    curve.map(move |(i, s)| (x_scale_fn(i, sample_rate), y_scale_fn(s, 1.0))),
                )
                .color(iced::Color::from_rgb8(255, 0, 0)),
            )
            .push_series(
                point_series(window.handles.iter().map(move |handle| {
                    let x = x_scale_fn(handle.x, sample_rate);
                    let y = y_scale_fn(handle.y, 1.0);
                    let style = handle.style.clone();

                    WindowHandle { x, y, style }
                }))
                .with_id(SeriesId::PointList)
                .color(iced::Color::from_rgb8(0, 255, 0))
                .style_for_each(|item| item.style.clone()),
            )
            .on_press(|state: &pliced::chart::State<SeriesId>| {
                let id = state.items().and_then(|l| l.first().map(|i| i.1));
                Message::Window(WindowOperation::MouseDown(id, state.get_offset()))
            })
            .on_move(|state: &pliced::chart::State<SeriesId>| {
                let id = state.items().and_then(|l| l.first().map(|i| i.1));
                Message::Window(WindowOperation::OnMove(id, state.get_offset()))
            })
            .on_release(|state: &pliced::chart::State<SeriesId>| {
                Message::Window(WindowOperation::MouseUp(state.get_offset()))
            })
    } else {
        chart
    };

    chart.into()
}

fn list_category<'a>(name: &'a str, content: Element<'a, Message>) -> Element<'a, Message> {
    let header = row!(text(name), horizontal_space()).align_y(Alignment::Center);

    column!(header, horizontal_rule(1), content)
        .width(Length::Fill)
        .spacing(5)
        .into()
}

impl From<&WindowHandle> for (f32, f32) {
    fn from(handle: &WindowHandle) -> Self {
        (handle.x, handle.y)
    }
}

impl From<WindowHandle> for (f32, f32) {
    fn from(handle: WindowHandle) -> Self {
        (handle.x, handle.y)
    }
}
//async fn windowed_median(data: &mut [f32]) -> f32 {
//    const WINDOW_SIZE: usize = 512;
//
//    let mut mean_of_median = 0f32;
//    let window_count = data.len() / WINDOW_SIZE;
//
//    for window_num in 0..window_count {
//        let start = window_num * WINDOW_SIZE;
//        let end = start + WINDOW_SIZE;
//
//        let window = &mut data[start..end];
//        window.sort_by(|a, b| a.partial_cmp(b).unwrap());
//
//        mean_of_median += window[256];
//    }
//
//    mean_of_median / window_count as f32
//}
//
//async fn estimate_noise_floor_border(noise_floor: f32, data: &[f32]) -> usize {
//    const WINDOW_SIZE: usize = 1024 * 2;
//
//    let window_count = data.len() / WINDOW_SIZE;
//    let nf_border = 0;
//
//    for window_num in 0..window_count {
//        let start = window_num * WINDOW_SIZE;
//        let end = start + WINDOW_SIZE;
//
//        let window = &data[start..end];
//
//        let mean = window.iter().fold(0f32, |acc, e| acc + e) / WINDOW_SIZE as f32;
//        if mean < noise_floor {
//            return end;
//        }
//    }
//
//    nf_border
//}
