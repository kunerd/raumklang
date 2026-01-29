use std::{sync::Arc, time::SystemTime};

use crate::{
    data::impulse_response,
    data::{self, SampleRate},
    icon,
    widget::sidebar,
};

use chrono::{DateTime, Utc};
use iced::{
    task::Sipper,
    widget::{button, column, container, right, row, rule, stack, text},
    Color, Element,
    Length::{Fill, Shrink},
    Task,
};

#[derive(Debug, Clone)]
pub enum Message {
    Select,
    Save,
}

#[derive(Debug, Clone)]
pub struct ImpulseResponse {
    pub sample_rate: SampleRate,
    pub data: Vec<f32>,
    pub origin: data::ImpulseResponse,
}

#[derive(Debug, Clone)]
pub enum State {
    Computing(data::ImpulseResponse),
    Computed(ImpulseResponse),
}

impl State {
    pub(crate) fn from_data(impulse_response: data::ImpulseResponse) -> State {
        match ImpulseResponse::from_data(&impulse_response) {
            Some(ir) => State::Computed(ir),
            None => State::Computing(impulse_response),
        }
    }

    pub(crate) fn progress(&self) -> impulse_response::Progress {
        match self {
            State::Computing(ir) => ir.progress(),
            State::Computed(_) => impulse_response::Progress::Computed,
        }
    }

    pub(crate) fn inner(&self) -> Option<&ImpulseResponse> {
        match self {
            State::Computing(_) => None,
            State::Computed(ref impulse_response) => Some(impulse_response),
        }
    }

    pub(crate) fn compute(
        &self,
        loopback: &raumklang_core::Loopback,
        measurement: &raumklang_core::Measurement,
    ) -> Option<impl Sipper<data::ImpulseResponse, data::ImpulseResponse>> {
        match self {
            State::Computing(ref impulse_response) => {
                impulse_response.clone().compute(loopback, measurement)
            }
            State::Computed(_) => None,
        }
    }
}

impl Default for State {
    fn default() -> Self {
        Self::Computing(data::ImpulseResponse::default())
    }
}

impl ImpulseResponse {
    pub fn from_data(data: &data::ImpulseResponse) -> Option<Self> {
        let impulse_response = data.inner()?;

        let max = impulse_response
            .data
            .iter()
            .map(|s| s.re.abs())
            .max_by(f32::total_cmp)
            .unwrap();

        let normalized = impulse_response
            .data
            .iter()
            .map(|s| s.re)
            .map(|s| s / max.abs())
            .collect();

        Some(Self {
            sample_rate: SampleRate::new(impulse_response.sample_rate),
            data: normalized,
            origin: data.clone(),
        })
    }
}

pub fn view<'a>(
    name: &'a str,
    date_time: SystemTime,
    progress: Option<impulse_response::Progress>,
    active: bool,
) -> Element<'a, Message> {
    let entry = {
        let dt: DateTime<Utc> = date_time.into();
        let ir_btn = button(
            column![
                text(name).size(16).wrapping(text::Wrapping::WordOrGlyph),
                text!("{}", dt.format("%x %X")).size(10)
            ]
            .clip(true)
            .spacing(6),
        )
        .on_press_with(move || Message::Select)
        .width(Fill)
        .style(move |theme, status| {
            let base = button::subtle(theme, status);
            let background = theme.extended_palette().background;

            if active {
                base.with_background(background.weak.color)
            } else {
                base
            }
        });

        let save_btn = button(icon::download().size(10))
            .style(button::secondary)
            .on_press_with(move || Message::Save);

        let content = row![
            ir_btn,
            rule::vertical(1.0),
            right(save_btn).width(Shrink).padding([0, 6])
        ];

        sidebar::item(content, active)
    };

    match progress {
        Some(impulse_response::Progress::Computing) => {
            processing_overlay("Impulse Response", entry)
        }
        _ => entry,
    }
}

fn processing_overlay<'a, Message>(
    status: &'a str,
    entry: impl Into<Element<'a, Message>>,
) -> Element<'a, Message>
where
    Message: 'a,
{
    stack([
        container(entry).style(container::bordered_box).into(),
        container(column![text("Computing..."), text(status).size(12)])
            .center(Fill)
            .style(|theme| container::Style {
                border: container::rounded_box(theme).border,
                background: Some(iced::Background::Color(Color::from_rgba(
                    0.0, 0.0, 0.0, 0.8,
                ))),
                ..Default::default()
            })
            .into(),
    ])
    .into()
}
