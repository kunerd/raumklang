use crate::{
    data::{
        self,
        measurement::{self, FromFile},
    },
    delete_icon,
};

use pliced::chart::{line_series, Chart, Labels};
use raumklang_core::WavLoadError;

use iced::{
    widget::{
        self, button, column, container, horizontal_rule, horizontal_space, row, scrollable, text,
    },
    Alignment, Element, Length, Task,
};

use rfd::FileHandle;

use std::{path::PathBuf, sync::Arc};

pub struct Measurements {
    selected: Option<Selected>,
}

#[derive(Debug, Clone)]
pub enum Message {
    AddLoopback,
    RemoveLoopback,
    LoopbackSignalLoaded(Result<Arc<data::measurement::Loopback>, Error>),
    AddMeasurement,
    RemoveMeasurement(usize),
    MeasurementSignalLoaded(Result<Arc<data::Measurement>, Error>),
    Select(Selected),
}

#[derive(Debug, Clone)]
pub enum Selected {
    Loopback,
    Measurement(usize),
}

pub enum Action {
    LoopbackAdded(data::measurement::Loopback),
    RemoveLoopback,
    MeasurementAdded(data::Measurement),
    RemoveMeasurement(usize),
    Task(Task<Message>),
    None,
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum Error {
    #[error("error while loading file: {0}")]
    File(PathBuf, Arc<WavLoadError>),
    #[error("dialog closed")]
    DialogClosed,
}

impl Measurements {
    pub fn new() -> Self {
        Self { selected: None }
    }

    pub fn update(&mut self, msg: Message) -> Action {
        match msg {
            Message::AddLoopback => Action::Task(Task::perform(
                pick_file_and_load_signal("Loopback"),
                Message::LoopbackSignalLoaded,
            )),
            Message::RemoveLoopback => Action::RemoveLoopback,
            Message::LoopbackSignalLoaded(Ok(signal)) => match Arc::into_inner(signal) {
                Some(signal) => Action::LoopbackAdded(signal),
                None => Action::None,
            },
            Message::LoopbackSignalLoaded(Err(err)) => {
                dbg!(err);
                Action::None
            }
            Message::AddMeasurement => Action::Task(Task::perform(
                pick_file_and_load_signal("Measurement"),
                Message::MeasurementSignalLoaded,
            )),
            Message::RemoveMeasurement(id) => Action::RemoveMeasurement(id),
            Message::MeasurementSignalLoaded(Ok(signal)) => match Arc::into_inner(signal) {
                Some(signal) => Action::MeasurementAdded(signal),
                None => Action::None,
            },
            Message::MeasurementSignalLoaded(Err(err)) => {
                dbg!(err);
                Action::None
            }
            Message::Select(selected) => {
                self.selected = Some(selected);

                Action::None
            }
        }
    }

    pub fn view<'a>(&'a self, project: &'a data::Project) -> Element<'a, Message> {
        let sidebar = {
            let loopback = {
                let (msg, content) = match project.loopback.as_ref() {
                    Some(signal) => (
                        None,
                        loopback_list_entry(self.selected.as_ref(), signal).into(),
                    ),
                    None => (Some(Message::AddLoopback), horizontal_space().into()),
                };

                signal_list_category("Loopback", msg, content)
            };

            let measurements = {
                let content = if project.measurements.is_empty() {
                    horizontal_space().into()
                } else {
                    column(project.measurements.iter().enumerate().map(|(id, signal)| {
                        measurement_list_entry(id, signal, self.selected.as_ref())
                    }))
                    .spacing(3)
                    .into()
                };

                signal_list_category("Measurements", Some(Message::AddMeasurement), content)
            };

            container(scrollable(
                column![loopback, measurements].spacing(20).padding(10),
            ))
            .style(container::rounded_box)
        }
        .width(Length::FillPortion(1));

        let content = {
            let content: Element<_> = if project.has_no_measurements() {
                text("You need to load one loopback or measurement signal at least.").into()
            } else {
                let signal = self
                    .selected
                    .as_ref()
                    .and_then(|selection| match selection {
                        Selected::Loopback => project.loopback.as_ref().and_then(|s| {
                            if let measurement::State::Loaded(data) = &s.state {
                                Some(data.iter())
                            } else {
                                None
                            }
                        }),
                        Selected::Measurement(id) => project.measurements.get(*id).and_then(|s| {
                            if let measurement::State::Loaded(data) = &s.state {
                                Some(data.iter())
                            } else {
                                None
                            }
                        }),
                    });

                if let Some(signal) = signal {
                    Chart::<_, (), _>::new()
                        .width(Length::Fill)
                        .height(Length::Fill)
                        // .x_range(self.x_range.clone())
                        .x_labels(Labels::default().format(&|v| format!("{v:.0}")))
                        .y_labels(Labels::default().format(&|v| format!("{v:.1}")))
                        .push_series(
                            line_series(signal.copied().enumerate().map(|(i, s)| (i as f32, s)))
                                .color(iced::Color::from_rgb8(2, 125, 66)),
                        )
                        // .on_scroll(|state: &pliced::chart::State<()>| {
                        //     let pos = state.get_coords();
                        //     let delta = state.scroll_delta();
                        //     Message::ChartScroll(pos, delta)
                        // })
                        .into()
                } else {
                    text("Select a signal to view its data.").into()
                }
            };

            container(content).center(Length::FillPortion(4))
        };

        column!(row![sidebar, content]).padding(10).into()
    }
}

fn signal_list_category<'a>(
    name: &'a str,
    add_msg: Option<Message>,
    content: Element<'a, Message>,
) -> Element<'a, Message> {
    let header = row!(widget::text(name), horizontal_space()).align_y(Alignment::Center);

    let header = if let Some(msg) = add_msg {
        header.push(button("+").on_press(msg))
    } else {
        header
    };

    column!(header, horizontal_rule(1), content)
        .width(Length::Fill)
        .spacing(5)
        .into()
}

fn loopback_list_entry<'a>(
    selected: Option<&Selected>,
    signal: &'a data::measurement::Loopback,
) -> Element<'a, Message> {
    let (data_info, select_msg) = match &signal.state {
        measurement::State::NotLoaded => (None, None),
        measurement::State::Loaded(data) => {
            let samples = data.duration();
            let sample_rate = data.sample_rate() as f32;
            let info = column![
                text(format!("Samples: {}", samples)).size(12),
                text(format!("Duration: {} s", samples as f32 / sample_rate)).size(12),
            ];

            (Some(info), Some(Message::Select(Selected::Loopback)))
        }
    };

    let content = column![
        column![text(&signal.name).size(16)]
            .push_maybe(data_info)
            .spacing(5),
        horizontal_rule(3),
        row![
            horizontal_space(),
            button("...").style(button::secondary),
            button(delete_icon())
                .on_press(Message::RemoveLoopback)
                .style(button::danger)
        ]
        .spacing(3),
    ]
    .clip(true)
    .spacing(3);

    let style = if let Some(Selected::Loopback) = selected {
        button::primary
    } else {
        button::secondary
    };

    button(content)
        .on_press_maybe(select_msg)
        .style(style)
        .width(Length::Fill)
        .into()
}

fn measurement_list_entry<'a>(
    index: usize,
    signal: &'a data::Measurement,
    selected: Option<&Selected>,
) -> Element<'a, Message> {
    let (data_info, select_msg) = match &signal.state {
        measurement::State::NotLoaded => (None, None),
        measurement::State::Loaded(data) => {
            let samples = data.duration();
            let sample_rate = data.sample_rate() as f32;
            let info = column![
                text(format!("Samples: {}", samples)).size(12),
                text(format!("Duration: {} s", samples as f32 / sample_rate)).size(12),
            ];

            (
                Some(info),
                Some(Message::Select(Selected::Measurement(index))),
            )
        }
    };

    let content = column![
        column![text(&signal.name).size(16),]
            .push_maybe(data_info)
            .spacing(5),
        horizontal_rule(3),
        row![
            horizontal_space(),
            button("...").style(button::secondary),
            button(delete_icon())
                .on_press(Message::RemoveMeasurement(index))
                .style(button::danger)
        ]
        .spacing(3),
    ]
    .clip(true)
    .spacing(3);

    let style = match selected {
        Some(Selected::Measurement(selected)) if *selected == index => button::primary,
        _ => button::secondary,
    };

    button(content)
        .on_press_maybe(select_msg)
        .width(Length::Fill)
        .style(style)
        .into()
}

async fn pick_file_and_load_signal<T>(file_type: impl AsRef<str>) -> Result<Arc<T>, Error>
where
    T: FromFile + Send + 'static,
{
    let handle = pick_file(file_type).await?;
    measurement::load_from_file(handle.path())
        .await
        .map(Arc::new)
        .map_err(|err| Error::File(handle.path().to_path_buf(), Arc::new(err)))
}

async fn pick_file(file_type: impl AsRef<str>) -> Result<FileHandle, Error> {
    rfd::AsyncFileDialog::new()
        .set_title(format!("Choose {} file", file_type.as_ref()))
        .add_filter("wav", &["wav", "wave"])
        .add_filter("all", &["*"])
        .pick_file()
        .await
        .ok_or(Error::DialogClosed)
}
