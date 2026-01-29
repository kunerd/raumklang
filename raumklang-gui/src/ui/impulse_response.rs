use std::time::SystemTime;

use crate::{
    data::{self, SampleRate},
    icon,
    ui::impulse_response,
    widget::sidebar,
};

use chrono::{DateTime, Utc};
use iced::{
    widget::{button, column, container, right, row, rule, stack, text, text::Wrapping},
    Color, Element,
    Length::{Fill, Shrink},
};

#[derive(Debug, Clone)]
pub enum Message {
    Select,
    OpenSaveFileDialog,
}

#[derive(Debug, Clone)]
pub struct ImpulseResponse {
    pub sample_rate: SampleRate,
    pub data: Vec<f32>,
    pub origin: raumklang_core::ImpulseResponse,
}

impl ImpulseResponse {
    pub fn from_data(impulse_response: data::ImpulseResponse) -> Self {
        let max = impulse_response
            .origin
            .data
            .iter()
            .map(|s| s.re.abs())
            .max_by(f32::total_cmp)
            .unwrap();

        let normalized = impulse_response
            .origin
            .data
            .iter()
            .map(|s| s.re)
            .map(|s| s / max.abs())
            .collect();

        Self {
            sample_rate: SampleRate::new(impulse_response.origin.sample_rate),
            data: normalized,
            origin: impulse_response.origin,
        }
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
                text(name).size(16).wrapping(Wrapping::WordOrGlyph),
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
            .on_press_with(move || Message::OpenSaveFileDialog);

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

#[derive(Debug, Clone)]
pub enum Progress {
    None,
    Computing,
    Finished,
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
