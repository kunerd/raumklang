use crate::{
    data::{self, FromFile},
    delete_icon, Project,
};

use raumklang_core::WavLoadError;

use iced::{
    wgpu::core::pipeline::ColorStateError,
    widget::{
        self, button, column, container, horizontal_rule, horizontal_space, row, scrollable, text,
    },
    Alignment, Element, Length, Task,
};

use rfd::FileHandle;

use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

// use pliced::chart::{line_series, Chart, Labels};

// use raumklang_core::WavLoadError;

// use crate::{data, delete_icon, OfflineMeasurement};

pub struct Measurements {
    selected: Option<Selected>,
    // shift_key_pressed: bool,
    // x_max: Option<f32>,
    // x_range: RangeInclusive<f32>,
}

#[derive(Debug, Clone)]
pub enum Message {
    AddLoopback,
    RemoveLoopback,
    LoopbackSignalLoaded(Result<Arc<data::Loopback>, Error>),
    AddMeasurement,
    RemoveMeasurement(usize),
    MeasurementSignalLoaded(Result<Arc<data::Measurement>, Error>),
    Select(Selected), // LoadMeasurement,
                      // RemoveMeasurement(usize),
                      // LoadLoopbackMeasurement,
                      // RemoveLoopbackMeasurement,
                      // MeasurementSelected(SelectedMeasurement),
                      // ChartScroll(Option<Point>, Option<ScrollDelta>),
                      // ShiftKeyPressed,
                      // ShiftKeyReleased,
}

#[derive(Debug, Clone)]
pub enum Selected {
    Loopback,
    Measurement(usize),
}

pub enum Action {
    LoopbackAdded(data::Loopback),
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
        Self {
            selected: None,
            // shift_key_pressed: false,
            // x_max: Some(10.0),
            // x_range: 0.0..=10.0,
        }
    }

    pub fn update(
        &mut self,
        msg: Message,
        // loopback: Option<&data::Loopback>,
        // measurements: &data::Store<data::Measurement, OfflineMeasurement>,
        // measurements: &Vec<MeasurementState<data::Measurement, OfflineMeasurement>>,
    ) -> Action {
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
        // match msg {
        //     Message::LoadLoopbackMeasurement => (Task::none(), Some(Event::LoadLoopback)),
        //     Message::RemoveLoopbackMeasurement => (Task::none(), Some(Event::RemoveLoopback)),
        //     Message::LoadMeasurement => (Task::none(), Some(Event::Load)),
        //     Message::RemoveMeasurement(index) => {
        //         let event = measurements
        //             .get_loaded_id(index)
        //             .map(|id| Event::Remove(index, id));

        //         (Task::none(), event)
        //     }
        //     Message::MeasurementSelected(selected) => {
        //         let signal = match selected {
        //             SelectedMeasurement::Loopback => {
        //                 loopback.map(|l| raumklang_core::Measurement::from(l.0.data.clone()))
        //             }
        //             SelectedMeasurement::Measurement(id) => {
        //                 measurements.get(id).and_then(|m| match m {
        //                     data::MeasurementState::Loaded(m) => Some(m.data.clone()),
        //                     data::MeasurementState::NotLoaded(_) => None,
        //                 })
        //             }
        //         };

        //         self.x_range = signal.map_or(0.0..=10.0, |s| 0.0..=s.duration() as f32);
        //         self.x_max = Some(*self.x_range.end());
        //         self.selected = Some(selected);

        //         (Task::none(), None)
        //     }
        //     Message::ChartScroll(pos, scroll_delta) => {
        //         let Some(pos) = pos else {
        //             return (Task::none(), None);
        //         };

        //         let Some(ScrollDelta::Lines { y, .. }) = scroll_delta else {
        //             return (Task::none(), None);
        //         };

        //         match (self.shift_key_pressed, y.is_sign_positive()) {
        //             (true, true) => self.scroll_right(),
        //             (true, false) => self.scroll_left(),
        //             (false, true) => self.zoom_in(pos),
        //             (false, false) => self.zoom_out(pos),
        //         }

        //         (Task::none(), None)
        //     }
        //     Message::ShiftKeyPressed => {
        //         self.shift_key_pressed = true;
        //         (Task::none(), None)
        //     }
        //     Message::ShiftKeyReleased => {
        //         self.shift_key_pressed = false;
        //         (Task::none(), None)
        //     }
        // }
    }

    pub fn view<'a>(
        &'a self,
        project: &'a Project,
        // loopback: Option<&'a data::MeasurementState<data::Loopback, OfflineMeasurement>>,
        // measurements: &'a Vec<data::MeasurementState<data::Measurement, OfflineMeasurement>>,
    ) -> Element<'a, Message> {
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
            let content = if project.has_no_measurements() {
                text("You need to load one loopback or measurement signal at least.")
            } else {
                text("Not implemented, yet")
            };

            container(content).center(Length::FillPortion(4))
        };

        column!(row![sidebar, content]).padding(10).into()
        //         let measurements_list = collecting_list(self.selected.as_ref(), loopback, measurements);

        //         let side_menu =
        //             container(container(scrollable(measurements_list).height(Length::Fill)).padding(8))
        //                 .style(container::rounded_box);

        //         let signal = match self.selected {
        //             Some(SelectedMeasurement::Loopback) => loopback.and_then(|l| match l {
        //                 data::MeasurementState::Loaded(m) => Some(m.0.data.0.iter()),
        //                 data::MeasurementState::NotLoaded(_) | data::MeasurementState::Loading(_) => None,
        //             }),
        //             Some(SelectedMeasurement::Measurement(id)) => {
        //                 measurements.get(id).and_then(|m| match m {
        //                     data::MeasurementState::Loaded(signal) => Some(signal.data.iter()),
        //                     data::MeasurementState::NotLoaded(_) | data::MeasurementState::Loading(_) => {
        //                         None
        //                     }
        //                 })
        //             }
        //             None => None,
        //         };

        //         let content: Element<_> = if let Some(signal) = signal {
        //             Chart::<_, (), _>::new()
        //                 .width(Length::Fill)
        //                 .height(Length::Fill)
        //                 .x_range(self.x_range.clone())
        //                 .x_labels(Labels::default().format(&|v| format!("{v:.0}")))
        //                 .y_labels(Labels::default().format(&|v| format!("{v:.1}")))
        //                 .push_series(
        //                     line_series(signal.copied().enumerate().map(|(i, s)| (i as f32, s)))
        //                         .color(iced::Color::from_rgb8(2, 125, 66)),
        //                 )
        //                 .on_scroll(|state: &pliced::chart::State<()>| {
        //                     let pos = state.get_coords();
        //                     let delta = state.scroll_delta();

        //                     Message::ChartScroll(pos, delta)
        //                 })
        //                 .into()
        //         } else {
        //             widget::text("Please select a measurement.").into()
        //         };

        //         row!(
        //             side_menu.width(Length::FillPortion(1)),
        //             container(content)
        //                 .center(Length::FillPortion(4))
        //                 .width(Length::FillPortion(4))
        //         )
        //         .height(Length::Fill)
        //         .spacing(5)
        //         .into()
    }

    //     pub fn subscription(&self) -> Subscription<Message> {
    //         Subscription::batch(vec![
    //             keyboard::on_key_press(|key, _modifiers| match key {
    //                 keyboard::Key::Named(keyboard::key::Named::Shift) => Some(Message::ShiftKeyPressed),
    //                 _ => None,
    //             }),
    //             keyboard::on_key_release(|key, _modifiers| match key {
    //                 keyboard::Key::Named(keyboard::key::Named::Shift) => {
    //                     Some(Message::ShiftKeyReleased)
    //                 }
    //                 _ => None,
    //             }),
    //         ])
    //     }

    //     fn scroll_right(&mut self) {
    //         let old_viewport = self.x_range.clone();
    //         let length = old_viewport.end() - old_viewport.start();

    //         const SCROLL_FACTOR: f32 = 0.2;
    //         let offset = length * SCROLL_FACTOR;

    //         let mut new_end = old_viewport.end() + offset;
    //         if let Some(x_max) = self.x_max {
    //             let viewport_max = x_max + length / 2.0;
    //             if new_end > viewport_max {
    //                 new_end = viewport_max;
    //             }
    //         }

    //         let new_start = new_end - length;

    //         self.x_range = new_start..=new_end;
    //     }

    //     fn scroll_left(&mut self) {
    //         let old_viewport = self.x_range.clone();
    //         let length = old_viewport.end() - old_viewport.start();

    //         const SCROLL_FACTOR: f32 = 0.2;
    //         let offset = length * SCROLL_FACTOR;

    //         let mut new_start = old_viewport.start() - offset;
    //         let viewport_min = -(length / 2.0);
    //         if new_start < viewport_min {
    //             new_start = viewport_min;
    //         }
    //         let new_end = new_start + length;

    //         self.x_range = new_start..=new_end;
    //     }

    //     fn zoom_in(&mut self, position: iced::Point) {
    //         let old_viewport = self.x_range.clone();
    //         let old_len = old_viewport.end() - old_viewport.start();

    //         let center_scale: f32 = (position.x - old_viewport.start()) / old_len;

    //         // FIXME make configurable
    //         const ZOOM_FACTOR: f32 = 0.8;
    //         const LOWER_BOUND: f32 = 50.0;
    //         let mut new_len = old_len * ZOOM_FACTOR;
    //         if new_len < LOWER_BOUND {
    //             new_len = LOWER_BOUND;
    //         }

    //         let new_start = position.x - (new_len * center_scale);
    //         let new_end = new_start + new_len;
    //         self.x_range = new_start..=new_end;
    //     }

    //     fn zoom_out(&mut self, position: iced::Point) {
    //         let old_viewport = self.x_range.clone();
    //         let old_len = old_viewport.end() - old_viewport.start();

    //         let center_scale = (position.x - old_viewport.start()) / old_len;

    //         // FIXME make configurable
    //         const ZOOM_FACTOR: f32 = 1.2;
    //         let new_len = old_len * ZOOM_FACTOR;
    //         //if new_len >= self.max_len {
    //         //    new_len = self.max_len;
    //         //}

    //         let new_start = position.x - (new_len * center_scale);
    //         let new_end = new_start + new_len;
    //         self.x_range = new_start..=new_end;
    //     }
}

async fn pick_file_and_load_signal<T>(file_type: impl AsRef<str>) -> Result<Arc<T>, Error>
where
    T: FromFile + Send + 'static,
{
    let handle = pick_file(file_type).await?;
    load_signal_from_file(handle.path())
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

async fn load_signal_from_file<P, T>(path: P) -> Result<T, WavLoadError>
where
    T: FromFile + Send + 'static,
    P: AsRef<Path> + Send + Sync,
{
    let path = path.as_ref().to_owned();
    tokio::task::spawn_blocking(move || T::from_file(path))
        .await
        .map_err(|_err| WavLoadError::Other)?
}

// fn collecting_list<'a>(
//     selected: Option<&SelectedMeasurement>,
//     loopback: Option<&'a data::MeasurementState<data::Loopback, OfflineMeasurement>>,
//     measurements: &'a Vec<data::MeasurementState<data::Measurement, OfflineMeasurement>>,
// ) -> Element<'a, Message> {
//     let loopback_entry = {
//         let content: Element<_> = match &loopback {
//             Some(data::MeasurementState::Loading(signal)) => loading(signal),
//             Some(data::MeasurementState::Loaded(signal)) => loopback_list_entry(selected, signal),
//             Some(data::MeasurementState::NotLoaded(signal)) => {
//                 offline_signal_list_entry(signal, Message::RemoveLoopbackMeasurement)
//             }
//             None => widget::text("Please load a loopback signal.").into(),
//         };

//         let add_msg = loopback
//             .as_ref()
//             .map_or(Some(Message::LoadLoopbackMeasurement), |_| None);

//         signal_list_category("Loopback", add_msg, content)
//     };

//     let measurement_entries = {
//         let content: Element<_> = {
//             if measurements.is_empty() {
//                 widget::text("Please load a measurement.").into()
//             } else {
//                 let entries: Vec<Element<_>> = measurements
//                     .iter()
//                     .enumerate()
//                     .map(|(index, state)| match state {
//                         data::MeasurementState::Loading(signal) => loading(signal),
//                         data::MeasurementState::Loaded(signal) => {
//                             measurement_list_entry(selected, signal, index)
//                         }
//                         data::MeasurementState::NotLoaded(signal) => {
//                             offline_signal_list_entry(signal, Message::RemoveMeasurement(index))
//                         }
//                     })
//                     .collect();

//                 column(entries).spacing(5).into()
//             }
//         };

//         signal_list_category("Measurements", Some(Message::LoadMeasurement), content)
//     };

//     column!(loopback_entry, measurement_entries)
//         .spacing(10)
//         .into()
// }

// fn loading<'a>(signal: &'a OfflineMeasurement) -> Element<'a, Message> {
//     stack!(
//         column!(row![
//             widget::text(signal.name.as_ref().map(String::as_str).unwrap_or("Unkown")),
//             horizontal_space(),
//         ],),
//         container(text("Loading ..."))
//             .center(Length::Fill)
//             .style(|theme| container::Style {
//                 border: container::rounded_box(theme).border,
//                 background: Some(iced::Background::Color(Color::from_rgba(
//                     0.0, 0.0, 0.0, 0.8,
//                 ))),
//                 ..Default::default()
//             })
//     )
//     .into()
// }

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

// fn offline_signal_list_entry(
//     signal: &crate::OfflineMeasurement,
//     delete_msg: Message,
// ) -> Element<'_, Message> {
//     column!(row![
//         widget::text(signal.name.as_ref().map(String::as_str).unwrap_or("Unkown")),
//         horizontal_space(),
//         button(delete_icon())
//             .on_press(delete_msg)
//             .style(button::danger)
//     ],)
//     .into()
// }

fn loopback_list_entry<'a>(
    selected: Option<&Selected>,
    signal: &'a data::Loopback,
) -> Element<'a, Message> {
    let samples = signal.0.data.0.duration();
    let sample_rate = signal.0.data.0.sample_rate() as f32;
    let content = column![
        column![
            text(&signal.0.name).size(16),
            column![
                text(format!("Samples: {}", samples)).size(12),
                text(format!("Duration: {} s", samples as f32 / sample_rate)).size(12),
            ]
        ]
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
        .on_press(Message::Select(Selected::Loopback))
        .style(style)
        .width(Length::Fill)
        .into()
}

fn measurement_list_entry<'a>(
    index: usize,
    signal: &'a data::Measurement,
    selected: Option<&Selected>,
) -> Element<'a, Message> {
    let samples = signal.data.duration();
    let sample_rate = signal.data.sample_rate() as f32;
    let content = column![
        column![
            text(&signal.name).size(16),
            column![
                text!("Samples: {}", samples).size(12),
                text!("Duration: {} s", samples as f32 / sample_rate).size(12),
            ]
        ]
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
        .on_press(Message::Select(Selected::Measurement(index)))
        .width(Length::Fill)
        .style(style)
        .into()
}
