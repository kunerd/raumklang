use crate::data::RecentProjects;

use iced::{
    widget::{button, column, container, row, rule, scrollable, space, text},
    Element, Length,
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
                .into()
        });

    container(row![
        space::horizontal().width(Length::FillPortion(1)),
        row![
            column!(
                column![text("Project"), rule::horizontal(2)].spacing(4),
                column![
                    button("New").on_press(Message::New).width(Length::Fill),
                    button("Load ...")
                        .on_press(Message::Load)
                        .width(Length::Fill)
                ]
                .spacing(3)
            )
            .spacing(10)
            .padding(5)
            .width(Length::FillPortion(1)),
            column!(
                column![text("Open recent"), rule::horizontal(2)].spacing(4),
                scrollable(column(recent_project_entries).spacing(3))
            )
            .spacing(10)
            .padding(5)
            .width(Length::FillPortion(2))
        ]
        .spacing(20)
        .width(Length::FillPortion(2)),
        space::horizontal().width(Length::FillPortion(1)),
    ])
    .center(Length::Fill)
    .into()
}
