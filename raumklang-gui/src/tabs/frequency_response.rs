use iced::{
    widget::{column, container, row, text, toggler},
    Element,
    Length::{self, FillPortion},
};

use crate::data;

#[derive(Debug, Clone)]
pub enum Message {
    ListEntry(usize, ListEntryMessage),
}

#[derive(Debug, Clone)]
pub enum ListEntryMessage {
    ShowInGraphToggled(bool),
}

#[derive(Debug)]
pub struct FrequencyResponse {
    entries: Vec<ListEntry>,
}

#[derive(Debug, Default)]
struct ListEntry {
    name: String,
    show_in_graph: bool,
}

impl FrequencyResponse {
    pub fn new<'a>(measurement: impl Iterator<Item = &'a data::Measurement>) -> Self {
        let entries = measurement
            .map(|m| ListEntry {
                name: m.name.clone(),
                show_in_graph: true,
            })
            .collect();

        Self { entries }
    }

    pub fn view(&self) -> Element<'_, Message> {
        let entries = self
            .entries
            .iter()
            .enumerate()
            .map(|(i, e)| e.view().map(move |msg| Message::ListEntry(i, msg)));

        let list = container(column(entries).spacing(5).padding(8).width(FillPortion(1)))
            .style(container::rounded_box);
        let content = container(text("Not implemented")).center(Length::FillPortion(4));

        row![list, content].padding(10).into()
    }

    pub fn update(&mut self, message: Message) {
        match message {
            Message::ListEntry(id, message) => {
                if let Some(entry) = self.entries.get_mut(id) {
                    entry.update(message)
                }
            }
        }
    }
}

impl ListEntry {
    fn view(&self) -> Element<'_, ListEntryMessage> {
        let content = column![
            text(&self.name),
            toggler(self.show_in_graph).on_toggle(ListEntryMessage::ShowInGraphToggled)
        ];
        container(content).style(container::rounded_box).into()
    }

    fn update(&mut self, message: ListEntryMessage) {
        match message {
            ListEntryMessage::ShowInGraphToggled(state) => self.show_in_graph = state,
        }
    }
}
