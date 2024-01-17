// plotters-iced
//
// Iced backend for Plotters
// Copyright: 2022, Joylei <leingliu@gmail.com>
// License: MIT

extern crate iced;
extern crate plotters;
extern crate sysinfo;

use std::{cell::RefCell, ops::Range};

use iced::{
    alignment::{Horizontal, Vertical},
    event, executor, mouse, subscription,
    widget::{
        self,
        canvas::{self, Cache, Frame, Geometry},
        text_input, Column, Container, Text,
    },
    Application, Command, Element, Event, Font, Length, Settings, Size, Subscription, Theme,
};
use plotters::{coord::types::RangedCoordi64, prelude::*};
use plotters::{
    coord::{
        types::{RangedCoordf32, RangedCoordusize},
        ReverseCoordTranslate,
    },
    prelude::{Cartesian2d, ChartBuilder, ChartContext},
};
use plotters_backend::DrawingBackend;
use plotters_iced::{Chart, ChartWidget, Renderer};
use raumklang::ImpulseResponse;
use rustfft::{num_complex::Complex32, FftPlanner};

fn main() {
    State::run(Settings {
        antialiasing: true,
        default_font: Font::with_name("Noto Sans"),
        ..Settings::default()
    })
    .unwrap();
}

#[derive(Debug, Clone)]
enum Message {
    ComputeFrequencyResponse,
    ImpulseRespone(ImpulseResponseMessage),
    Back,
}

struct State {
    chart: ImpulseResponseChart,
    frequency_response: Option<Vec<f32>>,
}

impl Application for State {
    type Message = self::Message;
    type Executor = executor::Default;
    type Flags = ();
    type Theme = Theme;

    fn new(_flags: Self::Flags) -> (Self, Command<Self::Message>) {
        let loopback_path = "data/loopback.wav";
        let measurement_path = "data/measurement.wav";
        //let loopback_path = "data/loopback_edit.wav";
        //let measurement_path = "data/measurement_edit.wav";
        //let measurement_path = "data/measurement_edit_phase_invert.wav";
        let impulse_respone =
            ImpulseResponse::from_files(&loopback_path, &measurement_path).unwrap();
        let mut data: Vec<_> = impulse_respone
            .impulse_response
            .iter()
            .map(|s| s.re)
            .collect();

        //let data: Vec<_> = WindowBuilder::new(Window::Hann, Window::Tukey(0.25), 1024).build();
        let chart = TimeseriesChart::new(
            "Test".to_string(),
            data.into_iter(),
            Some(AmplitudeUnit::PercentFullScale),
        );

        let chart = ImpulseResponseChart::new(chart);
        (
            Self {
                chart,
                frequency_response: None,
            },
            Command::none(), //Command::batch([
                             //    font::load(include_bytes!("./fonts/notosans-regular.ttf").as_slice())
                             //        .map(Message::FontLoaded),
                             //    font::load(include_bytes!("./fonts/notosans-bold.ttf").as_slice())
                             //        .map(Message::FontLoaded),
                             //]),
        )
    }

    fn title(&self) -> String {
        "CPU Monitor Example".to_owned()
    }

    fn update(&mut self, message: Self::Message) -> Command<Self::Message> {
        match message {
            Message::ComputeFrequencyResponse => {
                let window = self.chart.builder.build();
                let mut windowed_ir: Vec<_> = self
                    .chart
                    .base_chart
                    .data
                    .iter()
                    .take(window.len())
                    .zip(window)
                    .map(|(c, w)| c * w)
                    .map(Complex32::from)
                    .collect();

                let mut planner = FftPlanner::<f32>::new();
                let fft = planner.plan_fft_forward(windowed_ir.len());

                fft.process(&mut windowed_ir);

                let frequency_response: Vec<_> = windowed_ir.iter().map(|s| s.re).collect();
                self.chart = ImpulseResponseChart::new(TimeseriesChart::new(
                    "FR".to_string(),
                    frequency_response
                        .clone()
                        .into_iter()
                        .take((1000.0 / (44100.0 / 27624.0)) as usize)
                        .skip(32),
                    Some(AmplitudeUnit::DezibelFullScale),
                ));
                self.frequency_response = Some(frequency_response);
            }
            Message::Back => self.frequency_response = None,
            Message::ImpulseRespone(msg) => self.chart.update_msg(msg),
        }

        Command::none()
    }

    fn view(&self) -> Element<'_, Self::Message> {
        let btn = if self.frequency_response.is_some() {
            widget::button("Back").on_press(Message::Back)
        } else {
            widget::button("FR").on_press(Message::ComputeFrequencyResponse)
        };

        widget::column!(btn, self.chart.view().map(Message::ImpulseRespone)).into()
    }

    fn subscription(&self) -> Subscription<Self::Message> {
        //const FPS: u64 = 50;
        //iced::time::every(Duration::from_millis(1000 / FPS)).map(|_| Message::Tick)
        subscription::events()
            .map(TimeSeriesMessage::EventOccured)
            .map(ImpulseResponseMessage::TimeSeries)
            .map(Message::ImpulseRespone)
    }
}

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

pub struct ImpulseResponseChart {
    builder: WindowBuilder,
    base_chart: TimeseriesChart,
    cache: Cache,

    left_window_width: String,
    right_window_width: String,
}

impl ImpulseResponseChart {
    pub fn new(base_chart: TimeseriesChart) -> Self {
        let builder = WindowBuilder::new(Window::Tukey(0.25), Window::Tukey(0.25), 27562);
        let left_window_width = builder.get_left_side_width().to_string();
        let right_window_width = builder.get_right_side_width().to_string();

        Self {
            builder,
            base_chart,
            cache: Cache::new(),
            left_window_width,
            right_window_width,
        }
    }

    pub fn view(&self) -> Element<ImpulseResponseMessage> {
        let header: Element<_> = widget::row!(
            Text::new("Window:"),
            text_input("", &self.left_window_width)
                .on_input(ImpulseResponseMessage::LeftWidthChanged)
                .on_submit(ImpulseResponseMessage::LeftWidthSubmit),
            text_input("", &self.right_window_width)
                .on_input(ImpulseResponseMessage::RightWidthChanged)
                .on_submit(ImpulseResponseMessage::RightWidthSubmit),
        )
        .into();

        Container::new(
            Column::new()
                .width(Length::Fill)
                .height(Length::Fill)
                .spacing(5)
                .push(header)
                .push(ChartWidget::new(self).height(Length::Fill)),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Horizontal::Center)
        .align_y(Vertical::Center)
        .padding(10)
        .into()
    }

    pub fn update_msg(&mut self, msg: ImpulseResponseMessage) {
        match msg {
            ImpulseResponseMessage::LeftWidthChanged(s) => self.left_window_width = s,
            ImpulseResponseMessage::LeftWidthSubmit => {
                if let Ok(width) = self.left_window_width.parse() {
                    self.builder.set_left_side_width(width);
                    self.cache.clear();
                }
            }
            ImpulseResponseMessage::RightWidthChanged(s) => self.right_window_width = s,
            ImpulseResponseMessage::RightWidthSubmit => {
                if let Ok(width) = self.right_window_width.parse() {
                    self.builder.set_right_side_width(width);
                    self.cache.clear();
                }
            }
            ImpulseResponseMessage::TimeSeries(msg) => self.base_chart.update_msg(msg),
        }
    }
}

#[derive(Debug, Clone)]
pub enum ImpulseResponseMessage {
    RightWidthChanged(String),
    RightWidthSubmit,
    LeftWidthChanged(String),
    LeftWidthSubmit,
    TimeSeries(TimeSeriesMessage),
}

impl Chart<ImpulseResponseMessage> for ImpulseResponseChart {
    type State = ();

    fn build_chart<DB: DrawingBackend>(&self, _state: &Self::State, builder: ChartBuilder<DB>) {
        let mut chart = self.base_chart.draw(builder);

        let window = self.builder.build();
        let max = window.iter().fold(f32::NEG_INFINITY, |a, b| a.max(*b));
        // FIXME: remove duplicate code with data processing
        let window = match &self.base_chart.amplitude_unit {
            Some(AmplitudeUnit::PercentFullScale) => {
                window.iter().map(|s| s / max * 100f32).collect()
            }
            Some(AmplitudeUnit::DezibelFullScale) => window
                .iter()
                .map(|s| {
                    let s = 20f32 * f32::log10(s / max);
                    // clip the signal
                    match (s.is_infinite(), s.is_sign_negative()) {
                        (true, true) => -100.0,
                        (true, false) => -100.0,
                        _ => s,
                    }
                })
                .collect(),
            None => window,
        };
        chart
            .draw_series(LineSeries::new(
                window.iter().enumerate().map(|(n, s)| (n as i64, *s)),
                &BLUE,
            ))
            .unwrap();
    }

    fn draw_chart<DB: DrawingBackend>(
        &self,
        state: &Self::State,
        root: DrawingArea<DB, plotters::coord::Shift>,
    ) {
        let builder = ChartBuilder::on(&root);
        self.build_chart(state, builder);
    }

    #[inline]
    fn draw<R: Renderer, F: Fn(&mut Frame)>(
        &self,
        renderer: &R,
        bounds: Size,
        draw_fn: F,
    ) -> Geometry {
        renderer.draw_cache(&self.cache, bounds, draw_fn)
    }

    fn update(
        &self,
        state: &mut Self::State,
        event: canvas::Event,
        bounds: iced::Rectangle,
        cursor: mouse::Cursor,
    ) -> (event::Status, Option<ImpulseResponseMessage>) {
        let (status, message) = self.base_chart.update(state, event, bounds, cursor);
        let msg = message.map(ImpulseResponseMessage::TimeSeries);

        (status, msg)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AmplitudeUnit {
    #[default]
    PercentFullScale,
    DezibelFullScale,
}

impl AmplitudeUnit {
    const ALL: [AmplitudeUnit; 2] = [
        AmplitudeUnit::PercentFullScale,
        AmplitudeUnit::DezibelFullScale,
    ];
}

impl std::fmt::Display for AmplitudeUnit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                AmplitudeUnit::PercentFullScale => "%FS",
                AmplitudeUnit::DezibelFullScale => "dbFS",
            }
        )
    }
}

#[derive(Debug, Clone)]
pub enum TimeSeriesMessage {
    MouseEvent(mouse::Event, iced::Point),
    EventOccured(Event),
    AmplitudeUnitChanged(AmplitudeUnit),
}

pub struct TimeseriesChart {
    shift: bool,
    cache: Cache,
    name: String,
    data: Vec<f32>,
    processed_data: Vec<f32>,
    min: f32,
    max: f32,
    viewport: Range<i64>,
    spec: RefCell<Option<Cartesian2d<RangedCoordi64, RangedCoordf32>>>,
    amplitude_unit: Option<AmplitudeUnit>,
}

impl TimeseriesChart {
    fn new(
        name: String,
        data: impl Iterator<Item = f32>,
        amplitude_unit: Option<AmplitudeUnit>,
    ) -> Self {
        let data: Vec<_> = data.collect();
        let viewport = 0..data.len() as i64;
        let mut chart = Self {
            name,
            data,
            min: f32::NEG_INFINITY,
            max: f32::INFINITY,
            processed_data: vec![],
            cache: Cache::new(),
            viewport,
            spec: RefCell::new(None),
            amplitude_unit,
            shift: false,
        };
        chart.process_data();
        chart
    }

    fn view(&self) -> Element<TimeSeriesMessage> {
        let header = widget::row!(
            Text::new(&self.name),
            widget::pick_list(
                &AmplitudeUnit::ALL[..],
                self.amplitude_unit,
                TimeSeriesMessage::AmplitudeUnitChanged
            ),
        );

        Container::new(
            Column::new()
                .width(Length::Fill)
                .height(Length::Fill)
                .spacing(5)
                .push(header)
                .push(ChartWidget::new(self).height(Length::Fill)),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Horizontal::Center)
        .align_y(Vertical::Center)
        .into()
    }

    fn update_msg(&mut self, msg: TimeSeriesMessage) {
        match msg {
            TimeSeriesMessage::MouseEvent(evt, point) => match evt {
                //mouse::Event::CursorEntered => todo!(),
                //mouse::Event::CursorLeft => todo!(),
                //mouse::Event::CursorMoved { position } => todo!(),
                //mouse::Event::ButtonPressed(_) => todo!(),
                //mouse::Event::ButtonReleased(_) => todo!(),
                mouse::Event::WheelScrolled {
                    delta: mouse::ScrollDelta::Lines { y, .. },
                } => {
                    match self.shift {
                        true => {
                            // y is always zero in iced 0.10
                            if y.is_sign_positive() {
                                self.scroll_right();
                            } else {
                                self.scroll_left();
                            }
                        }
                        false => {
                            // y is always zero in iced 0.10
                            if y.is_sign_positive() {
                                self.zoom_in(point);
                            } else {
                                self.zoom_out(point);
                            }
                        }
                    }
                }
                _ => {}
            },
            TimeSeriesMessage::EventOccured(event) => {
                if let Event::Keyboard(event) = event {
                    match event {
                        iced::keyboard::Event::KeyPressed {
                            key_code,
                            modifiers: _,
                        } => match key_code {
                            iced::keyboard::KeyCode::LShift => self.shift = true,
                            iced::keyboard::KeyCode::RShift => self.shift = true,
                            _ => {}
                        },
                        iced::keyboard::Event::KeyReleased {
                            key_code,
                            modifiers: _,
                        } => match key_code {
                            iced::keyboard::KeyCode::LShift => self.shift = false,
                            iced::keyboard::KeyCode::RShift => self.shift = false,
                            _ => {}
                        },
                        _ => {}
                    }
                }
            }
            TimeSeriesMessage::AmplitudeUnitChanged(u) => self.set_amplitude_unit(u),
        }
    }

    fn zoom_in(&mut self, p: iced::Point) {
        if let Some(spec) = self.spec.borrow().as_ref() {
            let cur_pos = spec.reverse_translate((p.x as i32, p.y as i32));

            if let Some((x, ..)) = cur_pos {
                let old_viewport = self.viewport.clone();
                let old_len = old_viewport.end - old_viewport.start;

                let center_scale = (x - old_viewport.start) as f32 / old_len as f32;

                // FIXME make configurable
                const ZOOM_FACTOR: f32 = 0.8;
                const LOWER_BOUND: i64 = 256;
                let mut new_len = (old_len as f32 * ZOOM_FACTOR) as i64;
                if new_len < LOWER_BOUND {
                    new_len = LOWER_BOUND;
                }

                let new_start = x.saturating_sub((new_len as f32 * center_scale) as i64);
                let new_end = new_start + new_len;
                self.viewport = new_start..new_end;

                self.cache.clear();
            }
        }
    }

    fn zoom_out(&mut self, p: iced::Point) {
        if let Some(spec) = self.spec.borrow().as_ref() {
            let cur_pos = spec.reverse_translate((p.x as i32, p.y as i32));

            if let Some((x, ..)) = cur_pos {
                let old_viewport = self.viewport.clone();
                let old_len = old_viewport.end - old_viewport.start;

                let center_scale = (x - old_viewport.start) as f32 / old_len as f32;

                // FIXME make configurable
                const ZOOM_FACTOR: f32 = 1.2;
                let mut new_len = (old_len as f32 * ZOOM_FACTOR) as i64;
                if new_len >= self.data.len() as i64 {
                    new_len = self.data.len() as i64;
                }

                let new_start = x.saturating_sub((new_len as f32 * center_scale) as i64);
                let new_end = new_start + new_len;
                self.viewport = new_start..new_end;

                self.cache.clear();
            }
        }
    }

    fn scroll_right(&mut self) {
        let old_viewport = self.viewport.clone();
        let length = old_viewport.end - old_viewport.start;

        const SCROLL_FACTOR: f32 = 0.2;
        let offset = (length as f32 * SCROLL_FACTOR) as i64;

        let mut new_end = old_viewport.end.saturating_add(offset);
        if new_end > self.data.len() as i64 {
            new_end = self.data.len() as i64;
        }

        let new_start = new_end - length;

        self.viewport = new_start..new_end;

        self.cache.clear();
    }

    fn scroll_left(&mut self) {
        let old_viewport = self.viewport.clone();
        let length = old_viewport.end - old_viewport.start;

        const SCROLL_FACTOR: f32 = 0.2;
        let offset = (length as f32 * SCROLL_FACTOR) as i64;

        let new_start = old_viewport.start.saturating_sub(offset);
        let new_end = new_start + length;

        self.viewport = new_start..new_end;

        self.cache.clear();
    }

    fn process_data(&mut self) {
        //let max = self.data.iter().fold(f32::NEG_INFINITY, |a, b| a.max(*b));
        let max = self
            .data
            .iter()
            .map(|s| s.powi(2).sqrt())
            .fold(f32::NEG_INFINITY, |a, b| a.max(b));

        // FIXME: precompute on amplitude change
        self.processed_data = match &self.amplitude_unit {
            Some(AmplitudeUnit::PercentFullScale) => {
                self.data.iter().map(|s| s / max * 100f32).collect()
            }
            Some(AmplitudeUnit::DezibelFullScale) => self
                .data
                .iter()
                .map(|s| 20f32 * f32::log10(s.abs() / max))
                .collect(),
            None => self.data.clone(),
        };

        self.min = self
            .processed_data
            .iter()
            .fold(f32::INFINITY, |a, b| a.min(*b));
        self.max = self
            .processed_data
            .iter()
            .fold(f32::NEG_INFINITY, |a, b| a.max(*b));

        self.cache.clear();
    }

    fn set_amplitude_unit(&mut self, u: AmplitudeUnit) {
        self.amplitude_unit = Some(u);

        self.process_data();
    }

    fn draw<'a, DB: DrawingBackend>(
        &'a self,
        mut builder: ChartBuilder<'a, 'a, DB>,
    ) -> ChartContext<DB, Cartesian2d<RangedCoordi64, RangedCoordf32>> {
        use plotters::prelude::*;

        let mut chart = builder
            .margin(5)
            .x_label_area_size(30)
            .y_label_area_size(30)
            .build_cartesian_2d(self.viewport.clone(), self.min..self.max)
            .unwrap();

        chart
            .draw_series(LineSeries::new(
                self.processed_data
                    .iter()
                    .enumerate()
                    .map(|(n, s)| (n as i64, *s)),
                &RED,
            ))
            .unwrap();

        chart
            .configure_mesh()
            .disable_mesh()
            //.disable_axes()
            .draw()
            .unwrap();

        *self.spec.borrow_mut() = Some(chart.as_coord_spec().clone());

        chart
    }
}

impl Chart<TimeSeriesMessage> for TimeseriesChart {
    type State = ();
    // fn update(
    //     &mut self,
    //     event: Event,
    //     bounds: Rectangle,
    //     cursor: Cursor,
    // ) -> (event::Status, Option<Message>) {
    //     self.cache.clear();
    //     (event::Status::Ignored, None)
    // }

    #[inline]
    fn draw<R: Renderer, F: Fn(&mut Frame)>(
        &self,
        renderer: &R,
        bounds: Size,
        draw_fn: F,
    ) -> Geometry {
        renderer.draw_cache(&self.cache, bounds, draw_fn)
    }

    fn build_chart<DB: DrawingBackend>(&self, _state: &Self::State, mut chart: ChartBuilder<DB>) {
        self.draw(chart);
    }

    fn update(
        &self,
        _state: &mut Self::State,
        event: canvas::Event,
        bounds: iced::Rectangle,
        cursor: mouse::Cursor,
    ) -> (event::Status, Option<TimeSeriesMessage>) {
        if let mouse::Cursor::Available(point) = cursor {
            match event {
                canvas::Event::Mouse(evt) if bounds.contains(point) => {
                    let p_origin = bounds.position();
                    let p = point - p_origin;
                    return (
                        event::Status::Captured,
                        Some(TimeSeriesMessage::MouseEvent(
                            evt,
                            iced::Point::new(p.x, p.y),
                        )),
                    );
                }
                _ => {}
            }
        }
        (event::Status::Ignored, None)
    }
}
