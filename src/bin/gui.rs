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
        Column, Container, Text,
    },
    Application, Command, Element, Event, Font, Length, Settings, Size, Subscription, Theme,
};
use plotters::{
    coord::{
        types::{RangedCoordf32, RangedCoordusize},
        ReverseCoordTranslate,
    },
    prelude::{Cartesian2d, ChartBuilder},
};
use plotters_backend::DrawingBackend;
use plotters_iced::{Chart, ChartWidget, Renderer};
use raumklang::ImpulseResponse;

fn main() {
    State::run(Settings {
        antialiasing: true,
        default_font: Font::with_name("Noto Sans"),
        ..Settings::default()
    })
    .unwrap();
}

#[derive(Debug)]
enum Message {
    MouseEvent(mouse::Event, iced::Point),
    EventOccured(Event),
    AmplitudeUnitChanged(AmplitudeUnit),
}

struct State {
    shift: bool,
    chart: SamplesChart,
}

impl Application for State {
    type Message = self::Message;
    type Executor = executor::Default;
    type Flags = ();
    type Theme = Theme;

    fn new(_flags: Self::Flags) -> (Self, Command<Self::Message>) {
        let loopback_path = "data/loopback.wav";
        let measurement_path = "data/measurement.wav";
        let impulse_respone =
            ImpulseResponse::from_files(&loopback_path, &measurement_path).unwrap();
        //let data: Vec<_> = impulse_respone
        //    .impulse_response
        //    .iter()
        //    .map(|s| s.re)
        //    .collect();
        let hann = HannWindow::new(500).data;
        let tukey = TukeyWindow::new(500, 0.25).data;
        let mut data: Vec<f32> = hann.into_iter().take(250).collect();
        data.extend(&tukey[250..(500.0 * 0.75) as usize]);

        let chart = SamplesChart::new("Test".to_string(), data.into_iter(), None);
        (
            Self {
                chart,
                shift: false,
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
            Message::MouseEvent(evt, point) => match evt {
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
                                self.chart.scroll_right();
                            } else {
                                self.chart.scroll_left();
                            }
                        }
                        false => {
                            // y is always zero in iced 0.10
                            if y.is_sign_positive() {
                                self.chart.zoom_in(point);
                            } else {
                                self.chart.zoom_out(point);
                            }
                        }
                    }
                }
                _ => {}
            },
            Message::EventOccured(event) => {
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
            Message::AmplitudeUnitChanged(u) => self.chart.set_amplitude_unit(u),
        }

        Command::none()
    }

    fn view(&self) -> Element<'_, Self::Message> {
        self.chart.view()
    }

    fn subscription(&self) -> Subscription<Self::Message> {
        //const FPS: u64 = 50;
        //iced::time::every(Duration::from_millis(1000 / FPS)).map(|_| Message::Tick)
        subscription::events().map(Message::EventOccured)
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
            self.width - self.left_side_width - self.right_side_width
        ]);
        window.append(&mut right);

        window
    }
}

fn create_window(window_type: &Window, width: usize) -> Vec<f32> {
    match window_type {
        Window::Hann => HannWindow::new(width).data,
        Window::Tukey(a) => TukeyWindow::new(width, *a).data,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum AmplitudeUnit {
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

struct SamplesChart {
    cache: Cache,
    name: String,
    data: Vec<f32>,
    processed_data: Vec<f32>,
    min: f32,
    max: f32,
    viewport: Range<usize>,
    spec: RefCell<Option<Cartesian2d<RangedCoordusize, RangedCoordf32>>>,
    amplitude_unit: Option<AmplitudeUnit>,
}

impl SamplesChart {
    fn new(
        name: String,
        data: impl Iterator<Item = f32>,
        amplitude_unit: Option<AmplitudeUnit>,
    ) -> Self {
        let data: Vec<_> = data.collect();
        let viewport = 0..data.len();
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
        };
        chart.process_data();
        chart
    }

    fn view(&self) -> Element<Message> {
        let header = widget::row!(
            Text::new(&self.name),
            widget::pick_list(
                &AmplitudeUnit::ALL[..],
                self.amplitude_unit,
                Message::AmplitudeUnitChanged
            )
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

    fn zoom_in(&mut self, p: iced::Point) {
        if let Some(spec) = self.spec.borrow().as_ref() {
            let cur_pos = spec.reverse_translate((p.x as i32, p.y as i32));

            if let Some((x, ..)) = cur_pos {
                let old_viewport = self.viewport.clone();
                let old_len = old_viewport.end - old_viewport.start;

                let center_scale = (x - old_viewport.start) as f32 / old_len as f32;

                // FIXME make configurable
                const ZOOM_FACTOR: f32 = 0.8;
                const LOWER_BOUND: usize = 256;
                let mut new_len = (old_len as f32 * ZOOM_FACTOR) as usize;
                if new_len < LOWER_BOUND {
                    new_len = LOWER_BOUND;
                }

                let new_start = x.saturating_sub((new_len as f32 * center_scale) as usize);
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
                let mut new_len = (old_len as f32 * ZOOM_FACTOR) as usize;
                if new_len >= self.data.len() {
                    new_len = self.data.len();
                }

                let new_start = x.saturating_sub((new_len as f32 * center_scale) as usize);
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
        let offset = (length as f32 * SCROLL_FACTOR) as usize;

        let mut new_end = old_viewport.end.saturating_add(offset);
        if new_end > self.data.len() {
            new_end = self.data.len();
        }

        let new_start = new_end - length;

        self.viewport = new_start..new_end;

        self.cache.clear();
    }

    fn scroll_left(&mut self) {
        let old_viewport = self.viewport.clone();
        let length = old_viewport.end - old_viewport.start;

        const SCROLL_FACTOR: f32 = 0.2;
        let offset = (length as f32 * SCROLL_FACTOR) as usize;

        let new_start = old_viewport.start.saturating_sub(offset);
        let new_end = new_start + length;

        self.viewport = new_start..new_end;

        self.cache.clear();
    }

    fn process_data(&mut self) {
        let max = self.data.iter().fold(f32::NEG_INFINITY, |a, b| a.max(*b));

        // FIXME: precompute on amplitude change
        self.processed_data = match &self.amplitude_unit {
            Some(AmplitudeUnit::PercentFullScale) => {
                self.data.iter().map(|s| s / max * 100f32).collect()
            }
            Some(AmplitudeUnit::DezibelFullScale) => self
                .data
                .iter()
                .map(|s| 20f32 * f32::log10(s / max))
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
}

impl Chart<Message> for SamplesChart {
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
        use plotters::prelude::*;

        let mut chart = chart
            .margin(5)
            .x_label_area_size(30)
            .y_label_area_size(30)
            .build_cartesian_2d(self.viewport.clone(), self.min..self.max)
            .unwrap();

        chart.configure_mesh().draw().unwrap();

        chart
            .draw_series(LineSeries::new(
                self.processed_data.iter().enumerate().map(|(n, s)| (n, *s)),
                &RED,
            ))
            .unwrap();

        chart.configure_mesh().disable_mesh().draw().unwrap();

        *self.spec.borrow_mut() = Some(chart.as_coord_spec().clone());
    }

    fn update(
        &self,
        _state: &mut Self::State,
        event: canvas::Event,
        bounds: iced::Rectangle,
        cursor: mouse::Cursor,
    ) -> (event::Status, Option<Message>) {
        if let mouse::Cursor::Available(point) = cursor {
            match event {
                canvas::Event::Mouse(evt) if bounds.contains(point) => {
                    let p_origin = bounds.position();
                    let p = point - p_origin;
                    return (
                        event::Status::Captured,
                        Some(Message::MouseEvent(evt, iced::Point::new(p.x, p.y))),
                    );
                }
                _ => {}
            }
        }
        (event::Status::Ignored, None)
    }
}
