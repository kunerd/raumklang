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
    components::window_settings::{self, WindowSettings},
    data,
    widgets::charts::{AmplitudeUnit, TimeSeriesUnit},
    OfflineMeasurement,
};

use super::compute_impulse_response;

#[derive(Debug, Clone)]
pub enum Message {
    MeasurementSelected(data::MeasurementId),
    ImpulseResponseComputed((data::MeasurementId, raumklang_core::ImpulseResponse)),
    WindowSettings(window_settings::Message),
    Chart(ChartOperation),
    Window(WindowOperation),
}

pub enum Event {
    ImpulseResponseComputed(data::MeasurementId, raumklang_core::ImpulseResponse),
}

pub struct ImpulseResponseTab {
    selected: Option<data::MeasurementId>,
    window_settings: WindowSettings,
    chart_data: ChartData,
    window: Window,
}

struct Window {
    curve: Vec<f32>,
    handles: Vec<WindowHandle>,
    dragging: Dragging,
    hovered_item: Option<usize>,
}

struct WindowHandle {
    x: f32,
    y: f32,
    style: PointStyle,
}

#[derive(Debug, Clone)]
enum ItemId {
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
    pub fn new(window_builder: &WindowBuilder) -> Self {
        let curve = window_builder.build();

        let left_side_left = 0.0;
        let left_side_right = left_side_left + window_builder.left_side_width as f32;
        let right_side_left = left_side_right + window_builder.offset as f32;
        let right_side_right = right_side_left + window_builder.right_side_width as f32;

        let handles = vec![
            WindowHandle::new(left_side_left, 0.0),
            WindowHandle::new(left_side_right, 1.0),
            WindowHandle::new(right_side_left, 1.0),
            WindowHandle::new(right_side_right, 0.0),
        ];

        Self {
            curve,
            handles,
            dragging: Dragging::None,
            hovered_item: None,
        }
    }

    fn apply(&mut self, window_builder: &mut WindowBuilder, operation: WindowOperation) {
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
                if id.is_none() {
                    if let Some(handle) = self.hovered_item.and_then(|id| self.handles.get_mut(id))
                    {
                        handle.style = PointStyle::default()
                    }
                }

                self.hovered_item = id;

                let Some(pos) = pos else {
                    return;
                };

                match self.dragging {
                    Dragging::CouldStillBeClick(id, prev_pos) => {
                        if prev_pos != pos {
                            if let Some(handle) = self.handles.get_mut(id) {
                                handle.x -= prev_pos.x - pos.x;
                            }
                            self.dragging = Dragging::ForSure(id, pos);
                        }
                    }
                    Dragging::ForSure(id, prev_pos) => {
                        if let Some(handle) = self.handles.get_mut(id) {
                            handle.x -= prev_pos.x - pos.x;
                        }
                        self.dragging = Dragging::ForSure(id, pos);
                    }
                    Dragging::None => {}
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
                        if let Some(handle) = self.handles.get_mut(id) {
                            handle.x -= prev_pos.x - pos.x;
                            handle.style = PointStyle::default();
                        }
                        self.dragging = Dragging::None;
                    }
                    Dragging::None => {}
                }
            }
        }
        let left_side_left = 0.0;
        let left_side_right = left_side_left + window_builder.left_side_width as f32;
        let right_side_left = left_side_right + window_builder.offset as f32;

        window_builder.left_side_width = self.handles[1].x.round() as usize;
        window_builder.right_side_width = (self.handles[3].x - left_side_right)
            .floor()
            .clamp(1.0, f32::MAX) as usize;
        self.curve = window_builder.build();

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
        let window_settings = WindowSettings::new(44_100);
        let window = Window::new(&window_settings.window_builder);

        Self {
            window_settings,
            selected: None,
            chart_data: ChartData::default(),
            window,
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
                    self.window_settings = WindowSettings::new(ir.data.len());

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
                self.window_settings = WindowSettings::new(ir.data.len());

                (Task::none(), Some(Event::ImpulseResponseComputed(id, ir)))
            }
            Message::Chart(operation) => {
                self.chart_data.apply(operation);

                (Task::none(), None)
            }
            Message::WindowSettings(msg) => {
                self.window_settings.update(msg);

                (Task::none(), None)
            }
            Message::Window(operation) => {
                self.window
                    .apply(&mut self.window_settings.window_builder, operation);
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

            let chart = chart_view(&self.chart_data, impulse_response, &self.window);
            let window_settings = if self.chart_data.show_window {
                Some(self.window_settings.view().map(Message::WindowSettings))
            } else {
                None
            };
            container(column![chart_menu, chart].push_maybe(window_settings))
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
    window: &'a Window,
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
        .y_labels(Labels::default().format(&|v| format!("{v:.2}")))
        .push_series(line_series(series).color(iced::Color::from_rgb8(2, 125, 66)));

    let chart =
        if chart_data.show_window {
            chart
                .push_series(
                    line_series(window.curve.iter().copied().enumerate().map(move |(i, s)| {
                        (x_scale_fn(i as f32, sample_rate), y_scale_fn(s, max))
                    }))
                    .color(iced::Color::from_rgb8(255, 0, 0)),
                )
                .push_series(
                    point_series(window.handles.iter().map(move |handle| {
                        let x = x_scale_fn(handle.x, sample_rate);
                        let y = y_scale_fn(handle.y, max);
                        let style = handle.style.clone();

                        WindowHandle { x, y, style }
                    }))
                    .with_id(ItemId::PointList)
                    .color(iced::Color::from_rgb8(0, 255, 0))
                    .style(|item| item.style.clone()),
                )
                .on_press(|state: &pliced::chart::State<ItemId>| {
                    let id = state.items().and_then(|l| l.first().map(|i| i.1));
                    Message::Window(WindowOperation::MouseDown(id, state.get_offset()))
                })
                .on_move(|state: &pliced::chart::State<ItemId>| {
                    let id = state.items().and_then(|l| l.first().map(|i| i.1));
                    Message::Window(WindowOperation::OnMove(id, state.get_offset()))
                })
                .on_release(|state: &pliced::chart::State<ItemId>| {
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
