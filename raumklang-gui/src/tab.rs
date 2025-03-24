// pub mod measurements;

// use measurements::Measurements;

use std::{ffi::OsStr, fmt::Debug};

use iced::{
    widget::{button, column, container, horizontal_rule, horizontal_space, row, scrollable, text},
    Element, Length,
};
use interpolation::Lerp;

use crate::data::RecentProjects;
// pub mod impulse_response;
// pub mod frequency_response;

// pub use frequency_response::FrequencyResponse;
// pub use impulse_response::ImpulseResponseTab;

pub enum Tab {
    Loading,
    Landing,
    Measurements,
}

pub fn landing<'a, Message: 'a + Clone>(
    recent_projects: &'a RecentProjects,
) -> Element<'a, Message> {
    let recent_project_entries = recent_projects
        .iter()
        .filter_map(|p| p.file_name())
        .filter_map(OsStr::to_str)
        .map(|n| button(n).width(Length::Fill).into());

    container(row![
        horizontal_space().width(Length::FillPortion(1)),
        row![
            column!(
                column![text("Project"), horizontal_rule(2)].spacing(4),
                column![
                    button("New").width(Length::Fill),
                    button("Load ...").width(Length::Fill)
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

pub fn loading<'a, Message: 'a>() -> Element<'a, Message> {
    container(text("Loading ...")).center(Length::Fill).into()
}

// async fn compute_impulse_response(
//     id: data::MeasurementId,
//     loopback: raumklang_core::Loopback,
//     measurement: raumklang_core::Measurement,
// ) -> (data::MeasurementId, raumklang_core::ImpulseResponse) {
//     let impulse_response = tokio::task::spawn_blocking(move || {
//         raumklang_core::ImpulseResponse::from_signals(&loopback, &measurement).unwrap()
//     })
//     .await
//     .unwrap();

//     (id, impulse_response)
// }
