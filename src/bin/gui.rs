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
        let samples = raumklang::LinearSineSweep::new(60, 500, 5, 0.8f32, 44100);
        let chart = SamplesChart::new("Test".to_string(), samples.into_iter());
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

struct SamplesChart {
    cache: Cache,
    name: String,
    data: Vec<f32>,
    viewport: Range<usize>,
    spec: RefCell<Option<Cartesian2d<RangedCoordusize, RangedCoordf32>>>,
}

impl SamplesChart {
    fn new(name: String, data: impl Iterator<Item = f32>) -> Self {
        let data: Vec<_> = data.collect();
        let viewport = 0..data.len();
        Self {
            name,
            data,
            cache: Cache::new(),
            viewport,
            spec: RefCell::new(None),
        }
    }

    fn view(&self) -> Element<Message> {
        Container::new(
            Column::new()
                .width(Length::Fill)
                .height(Length::Fill)
                .spacing(5)
                .push(Text::new(format!("WAV {}", self.name)))
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
            .build_cartesian_2d(self.viewport.clone(), -1.1f32..1.1f32)
            .unwrap();

        chart.configure_mesh().draw().unwrap();

        chart
            .draw_series(LineSeries::new(
                self.data.iter().enumerate().map(|(n, s)| (n, *s)),
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
