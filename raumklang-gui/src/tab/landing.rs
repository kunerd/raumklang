use crate::data::RecentProjects;

use iced::{
    widget::{button, column, container, horizontal_rule, horizontal_space, row, scrollable, text},
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
        .filter_map(|(i, p)| p.file_name().map(|f| (i, f)))
        .filter_map(|(i, p)| p.to_str().map(|f| (i, f)))
        .map(|(i, n)| {
            button(n)
                .on_press(Message::Recent(i))
                .width(Length::Fill)
                .into()
        });

    container(row![
        horizontal_space().width(Length::FillPortion(1)),
        row![
            column!(
                column![text("Project"), horizontal_rule(2)].spacing(4),
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
                column![text("Open recent"), horizontal_rule(2)].spacing(4),
                scrollable(column(recent_project_entries).spacing(3))
            )
            .spacing(10)
            .padding(5)
            .width(Length::FillPortion(2))
        ]
        .spacing(20)
        .width(Length::FillPortion(2)),
        horizontal_space().width(Length::FillPortion(1)),
    ])
    .center(Length::Fill)
    .into()
}
