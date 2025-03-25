use iced::{
    widget::{button, column, container, horizontal_rule, row, scrollable, text},
    Element, Length,
};

use crate::data;

#[derive(Debug, Clone)]
pub enum Message {
    Select(usize),
}

pub enum Action {
    ComputeImpulseResponse(usize),
}

pub struct ImpulseReponses {
    selected: Option<usize>,
}

impl ImpulseReponses {
    pub fn new() -> Self {
        Self { selected: None }
    }

    pub fn update(&mut self, message: Message) -> Action {
        match message {
            Message::Select(id) => {
                self.selected = Some(id);

                Action::ComputeImpulseResponse(id)
            }
        }
    }

    pub fn view<'a>(
        &'a self,
        impulse_responses: &'a [data::ImpulseResponse],
    ) -> Element<'a, Message> {
        let sidebar = {
            let header = {
                column!(text("For Measurements"), horizontal_rule(1))
                    .width(Length::Fill)
                    .spacing(5)
            };

            let measurements = impulse_responses.iter().enumerate().map(|(id, entry)| {
                let content = column![text(&entry.name).size(16),]
                    .spacing(5)
                    .clip(true)
                    .spacing(3);

                let style = match self.selected.as_ref() {
                    Some(selected) if *selected == id => button::primary,
                    _ => button::secondary,
                };

                button(content)
                    .on_press_with(move || Message::Select(id))
                    .width(Length::Fill)
                    .style(style)
                    .into()
            });

            container(scrollable(
                column![header, column(measurements).spacing(3)]
                    .spacing(10)
                    .padding(10),
            ))
            .style(container::rounded_box)
        }
        .width(Length::FillPortion(1));

        row![
            container(sidebar).width(Length::FillPortion(1)),
            container("Not implemented, yet.").center(Length::FillPortion(4))
        ]
        .into()
    }
}
