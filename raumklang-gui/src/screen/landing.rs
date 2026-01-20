use crate::data::RecentProjects;

use iced::{
    font,
    widget::{button, column, container, row, scrollable, space, text},
    Element, Font,
    Length::{self, Fill},
};

#[derive(Debug, Clone)]
pub enum Message {
    New,
    Load,
    Recent(usize),
}

pub fn landing<'a>(recent_projects: &'a RecentProjects) -> Element<'a, Message> {
    let recent_project_entries = recent_projects
        .iter()
        .enumerate()
        .rev()
        .filter_map(|(i, p)| p.file_name().map(|f| (i, f)))
        .filter_map(|(i, p)| p.to_str().map(|f| (i, f)))
        .map(|(i, n)| {
            button(n)
                .on_press(Message::Recent(i))
                .width(Length::Fill)
                .style(button::subtle)
                .into()
        });

    let bold = |label| {
        let mut font = Font::default();
        font.weight = font::Weight::Bold;

        text(label).font(font)
    };

    container(row![
        space::horizontal().width(Length::FillPortion(1)),
        row![
            column!(
                container(bold("Project"))
                    .padding(5)
                    .style(container::rounded_box)
                    .width(Fill),
                container(
                    column![
                        button("New")
                            .on_press(Message::New)
                            .width(Length::Fill)
                            .style(button::subtle),
                        button("Load ...")
                            .on_press(Message::Load)
                            .width(Length::Fill)
                            .style(button::subtle)
                    ]
                    .spacing(2)
                    .padding(1)
                )
                .style(container::bordered_box)
            )
            .width(Length::FillPortion(1)),
            column!(
                container(bold("Recent"))
                    .padding(5)
                    .style(container::rounded_box)
                    .width(Fill),
                container(scrollable(
                    column(recent_project_entries).spacing(2).padding(1)
                ))
                .style(container::bordered_box)
            )
            .width(Length::FillPortion(2))
        ]
        .spacing(20)
        .width(Length::FillPortion(2)),
        space::horizontal().width(Length::FillPortion(1)),
    ])
    .center(Length::Fill)
    .into()
}
