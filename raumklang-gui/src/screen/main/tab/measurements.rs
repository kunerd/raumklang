mod recording;
pub use recording::Recording;

use crate::{
    data::{
        self,
        measurement::{self, FromFile},
    },
    icon,
};

use prism::chart::{line_series, Chart, Labels};
use raumklang_core::WavLoadError;

use iced::{
    alignment::{Horizontal, Vertical},
    keyboard,
    mouse::ScrollDelta,
    widget::{
        button, column, container, horizontal_rule, horizontal_space, row, scrollable, text,
        text::Wrapping, Button,
    },
    Alignment, Element, Length, Point, Subscription, Task,
};

use rfd::FileHandle;

use std::{ops::RangeInclusive, path::PathBuf, sync::Arc};

pub struct Measurements {
    recording: Option<Recording>,
    selected: Option<Selected>,

    shift_key_pressed: bool,
    x_max: Option<f32>,
    x_range: Option<RangeInclusive<f32>>,
}

#[derive(Debug, Clone)]
pub enum Message {
    AddLoopback,
    RemoveLoopback,
    LoopbackSignalLoaded(Result<Arc<data::measurement::State<data::measurement::Loopback>>, Error>),
    AddMeasurement,
    RemoveMeasurement(usize),
    MeasurementSignalLoaded(Result<Arc<data::measurement::State<data::Measurement>>, Error>),
    Select(Selected),
    StartRecording(recording::Kind),
    ChartScroll(
        Option<Point>,
        Option<ScrollDelta>,
        Option<RangeInclusive<f32>>,
    ),
    ShiftKeyPressed,
    ShiftKeyReleased,
    Recording(recording::Message),
}

#[derive(Debug, Clone)]
pub enum Selected {
    Loopback,
    Measurement(usize),
}

pub enum Action {
    LoopbackAdded(data::measurement::State<data::measurement::Loopback>),
    RemoveLoopback,
    MeasurementAdded(data::measurement::State<data::Measurement>),
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
        Self {
            recording: None,
            selected: None,
            shift_key_pressed: false,
            x_max: None,
            x_range: None,
        }
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
                log::error(err);
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
                log::error(err);
                Action::None
            }
            Message::Select(selected) => {
                self.selected = Some(selected);

                Action::None
            }
            Message::ChartScroll(pos, scroll_delta, x_range) => {
                let Some(pos) = pos else {
                    return Action::None;
                };

                let Some(ScrollDelta::Lines { y, .. }) = scroll_delta else {
                    return Action::None;
                };

                if self.x_range.is_none() {
                    self.x_max = x_range.as_ref().map(|r| *r.end());
                    self.x_range = x_range;
                }

                match (self.shift_key_pressed, y.is_sign_positive()) {
                    (true, true) => self.scroll_right(),
                    (true, false) => self.scroll_left(),
                    (false, true) => self.zoom_in(pos),
                    (false, false) => self.zoom_out(pos),
                }

                Action::None
            }
            Message::ShiftKeyPressed => {
                self.shift_key_pressed = true;
                Action::None
            }
            Message::ShiftKeyReleased => {
                self.shift_key_pressed = false;
                Action::None
            }
            Message::StartRecording(kind) => {
                self.recording = Some(Recording::new(kind));
                Action::None
            }
            Message::Recording(message) => {
                let Some(recording) = &mut self.recording else {
                    return Action::None;
                };

                match recording.update(message) {
                    recording::Action::Cancel => {
                        self.recording = None;
                        Action::None
                    }
                    recording::Action::None => Action::None,
                    recording::Action::Task(task) => Action::Task(task.map(Message::Recording)),
                    recording::Action::Finished(result) => {
                        self.recording = None;

                        match result {
                            recording::Result::Loopback(loopback) => {
                                let details = measurement::Details {
                                    // FIXME auto generate
                                    name: "Loopback".to_string(),
                                    path: PathBuf::default(),
                                };
                                Action::LoopbackAdded(data::measurement::State::Loaded(
                                    data::measurement::Loopback::new(loopback, details),
                                ))
                            }
                            recording::Result::Measurement(measurement) => {
                                let details = measurement::Details {
                                    // FIXME auto generate
                                    name: "Measurement".to_string(),
                                    path: PathBuf::default(),
                                };
                                Action::MeasurementAdded(data::measurement::State::Loaded(
                                    data::Measurement::new(measurement, details),
                                ))
                            }
                        }
                    }
                }
            }
        }
    }

    pub fn view<'a>(&'a self, project: &'a data::Project) -> Element<'a, Message> {
        let sidebar =
            {
                let loopback =
                    Category::new("Loopback")
                        .push_button(
                            button("+")
                                .on_press_maybe(if project.loopback().is_none() {
                                    Some(Message::AddLoopback)
                                } else {
                                    None
                                })
                                .style(button::secondary),
                        )
                        .push_button(
                            button(icon::record())
                                .on_press(Message::StartRecording(recording::Kind::Loopback))
                                .style(button::secondary),
                        )
                        .push_entry_maybe(project.loopback().as_ref().map(|loopback| {
                            loopback_list_entry(self.selected.as_ref(), &loopback)
                        }));

                let measurements = Category::new("Measurements")
                    .push_button(
                        button("+")
                            .style(button::secondary)
                            .on_press(Message::AddMeasurement),
                    )
                    .push_button(
                        button(icon::record())
                            .on_press(Message::StartRecording(recording::Kind::Measurement))
                            .style(button::secondary),
                    )
                    .extend_entries(project.measurements.iter().enumerate().map(
                        |(id, measurement)| {
                            measurement_list_entry(id, measurement, self.selected.as_ref())
                        },
                    ));

                container(scrollable(
                    column![loopback, measurements].spacing(20).padding(10),
                ))
                .style(container::rounded_box)
            };

        let content: Element<_> = 'content: {
            if let Some(recording) = &self.recording {
                break 'content recording.view().map(Message::Recording);
            }

            let welcome_text = |base_text| {
                column![
                    text("Welcome").size(24),
                    column![
                        base_text,
                        row![
                            text("You can load signals from file by pressing [+] or"),
                            button(icon::record())
                                .style(button::secondary)
                                .on_press(Message::StartRecording(recording::Kind::Measurement))
                        ]
                        .spacing(8)
                        .align_y(Vertical::Center)
                    ]
                    .align_x(Horizontal::Center)
                    .spacing(10)
                ]
                .spacing(16)
                .align_x(Horizontal::Center)
                .into()
            };

            let content: Element<_> = if project.has_no_measurements() {
                welcome_text(text(
                    "You need to load at least one loopback or measurement signal.",
                ))
            } else {
                let signal = self
                    .selected
                    .as_ref()
                    .and_then(|selection| match selection {
                        Selected::Loopback => project.loopback().and_then(|s| {
                            if let data::measurement::State::Loaded(data) = &s {
                                Some(data.as_ref().iter())
                            } else {
                                None
                            }
                        }),
                        Selected::Measurement(id) => project
                            .measurements
                            .get(*id)
                            .and_then(measurement::State::signal),
                    });

                if let Some(signal) = signal {
                    Chart::<_, (), _>::new()
                        .width(Length::Fill)
                        .height(Length::Fill)
                        .x_range(self.x_range.clone().unwrap_or(0.0..=signal.len() as f32))
                        .x_labels(Labels::default().format(&|v| format!("{v:.0}")))
                        .y_labels(Labels::default().format(&|v| format!("{v:.1}")))
                        .push_series(
                            line_series(signal.copied().enumerate().map(|(i, s)| (i as f32, s)))
                                .color(iced::Color::from_rgb8(2, 125, 66)),
                        )
                        .on_scroll(|state| {
                            let pos = state.get_coords();
                            let delta = state.scroll_delta();
                            let x_range = state.x_range();
                            Message::ChartScroll(pos, delta, x_range)
                        })
                        .into()
                } else {
                    welcome_text(text("Select a signal to view its data."))
                }
            };

            container(content).center(Length::Fill).into()
        };

        column!(row![
            container(sidebar).width(Length::FillPortion(1)),
            container(content).width(Length::FillPortion(4))
        ]
        .spacing(8))
        .padding(10)
        .into()
    }

    pub fn subscription(&self) -> Subscription<Message> {
        let mut subscriptions = vec![];

        subscriptions.extend([
            keyboard::on_key_press(|key, _modifiers| match key {
                keyboard::Key::Named(keyboard::key::Named::Shift) => Some(Message::ShiftKeyPressed),
                _ => None,
            }),
            keyboard::on_key_release(|key, _modifiers| match key {
                keyboard::Key::Named(keyboard::key::Named::Shift) => {
                    Some(Message::ShiftKeyReleased)
                }
                _ => None,
            }),
        ]);

        if let Some(recording) = &self.recording {
            subscriptions.push(recording.subscription().map(Message::Recording));
        }

        Subscription::batch(subscriptions)
    }

    fn scroll_right(&mut self) {
        let Some(old_viewport) = self.x_range.clone() else {
            return;
        };

        let length = old_viewport.end() - old_viewport.start();

        const SCROLL_FACTOR: f32 = 0.2;
        let offset = length * SCROLL_FACTOR;

        let mut new_end = old_viewport.end() + offset;
        if let Some(x_max) = self.x_max {
            let viewport_max = x_max + length / 2.0;
            if new_end > viewport_max {
                new_end = viewport_max;
            }
        }

        let new_start = new_end - length;

        self.x_range = Some(new_start..=new_end);
    }

    fn scroll_left(&mut self) {
        let Some(old_viewport) = self.x_range.clone() else {
            return;
        };
        let length = old_viewport.end() - old_viewport.start();

        const SCROLL_FACTOR: f32 = 0.2;
        let offset = length * SCROLL_FACTOR;

        let mut new_start = old_viewport.start() - offset;
        let viewport_min = -(length / 2.0);
        if new_start < viewport_min {
            new_start = viewport_min;
        }
        let new_end = new_start + length;

        self.x_range = Some(new_start..=new_end);
    }

    fn zoom_in(&mut self, position: iced::Point) {
        let Some(old_viewport) = self.x_range.clone() else {
            return;
        };
        let old_len = old_viewport.end() - old_viewport.start();

        let center_scale: f32 = (position.x - old_viewport.start()) / old_len;

        // FIXME make configurable
        const ZOOM_FACTOR: f32 = 0.8;
        const LOWER_BOUND: f32 = 50.0;
        let mut new_len = old_len * ZOOM_FACTOR;
        if new_len < LOWER_BOUND {
            new_len = LOWER_BOUND;
        }

        let new_start = position.x - (new_len * center_scale);
        let new_end = new_start + new_len;
        self.x_range = Some(new_start..=new_end);
    }

    fn zoom_out(&mut self, position: iced::Point) {
        let Some(old_viewport) = self.x_range.clone() else {
            return;
        };
        let old_len = old_viewport.end() - old_viewport.start();

        let center_scale = (position.x - old_viewport.start()) / old_len;

        // FIXME make configurable
        const ZOOM_FACTOR: f32 = 1.2;
        let new_len = old_len * ZOOM_FACTOR;
        //if new_len >= self.max_len {
        //    new_len = self.max_len;
        //}

        let new_start = position.x - (new_len * center_scale);
        let new_end = new_start + new_len;
        self.x_range = Some(new_start..=new_end);
    }
}

fn loopback_list_entry<'a>(
    selected: Option<&Selected>,
    signal: &'a data::measurement::State<data::measurement::Loopback>,
) -> Element<'a, Message> {
    let (data_info, select_msg) = match &signal {
        data::measurement::State::NotLoaded(_) => (None, None),
        data::measurement::State::Loaded(data) => {
            let data = data.as_ref();
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
        column![text("Loopback").size(16)]
            .push_maybe(data_info)
            .spacing(5),
        horizontal_rule(3),
        row![
            horizontal_space(),
            button("...").style(button::secondary),
            button(icon::delete())
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
    signal: &'a data::measurement::State<data::Measurement>,
    selected: Option<&Selected>,
) -> Element<'a, Message> {
    let (data_info, select_msg) = match &signal {
        measurement::State::NotLoaded(_) => (None, None),
        measurement::State::Loaded(measurement) => {
            let measurement = measurement.as_ref();
            let samples = measurement.duration();
            let sample_rate = measurement.sample_rate() as f32;
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
        column![text(&signal.details().name).size(16),]
            .push_maybe(data_info)
            .spacing(5),
        horizontal_rule(3),
        row![
            horizontal_space(),
            button("...").style(button::secondary),
            button(icon::delete())
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

pub struct Category<'a, Message> {
    title: &'a str,
    entries: Vec<Element<'a, Message>>,
    buttons: Vec<Button<'a, Message>>,
}

impl<'a, Message> Category<'a, Message>
where
    Message: 'a + Clone,
{
    pub fn new(title: &'a str) -> Self {
        Self {
            title,
            entries: vec![],
            buttons: vec![],
        }
    }

    pub fn push_button(mut self, button: Button<'a, Message>) -> Self {
        self.buttons.push(button);
        self
    }

    pub fn push_entry(mut self, entry: impl Into<Element<'a, Message>>) -> Self {
        self.entries.push(entry.into());
        self
    }

    pub fn push_entry_maybe(self, entry: Option<impl Into<Element<'a, Message>>>) -> Self {
        if let Some(entry) = entry {
            self.push_entry(entry)
        } else {
            self
        }
    }

    pub fn extend_entries(self, entries: impl IntoIterator<Item = Element<'a, Message>>) -> Self {
        entries.into_iter().fold(self, Self::push_entry)
    }

    pub fn view(self) -> Element<'a, Message> {
        let header = row![container(text(self.title).wrapping(Wrapping::WordOrGlyph))
            .width(Length::Fill)
            .clip(true),]
        .extend(self.buttons.into_iter().map(|btn| btn.width(30).into()))
        .spacing(5)
        .padding(5)
        .align_y(Alignment::Center);

        column!(header, horizontal_rule(1))
            .extend(self.entries.into_iter())
            .width(Length::Fill)
            .spacing(5)
            .into()
    }
}

impl<'a, Message> From<Category<'a, Message>> for Element<'a, Message>
where
    Message: 'a + Clone,
{
    fn from(category: Category<'a, Message>) -> Self {
        category.view()
    }
}
