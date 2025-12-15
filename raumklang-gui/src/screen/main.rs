mod chart;
mod frequency_response;
mod impulse_response;
mod measurement;
mod recording;

use chrono::{DateTime, Utc};
use generic_overlay::generic_overlay::{dropdown_menu, dropdown_root};
use impulse_response::{ChartOperation, WindowSettings};
use recording::Recording;

use crate::{
    data::{
        self, project, spectral_decay, spectrogram, window, Project, RecentProjects, SampleRate,
        Samples, Window,
    },
    icon, load_project, log,
    screen::main::{chart::waveform, impulse_response::processing_overlay},
    ui,
    widget::sidebar,
    PickAndLoadError,
};

use raumklang_core::dbfs;

use iced::{
    alignment::{Horizontal, Vertical},
    futures::{FutureExt, TryFutureExt},
    keyboard,
    widget::{
        button, canvas, center, column, container, opaque, pick_list, right, row, rule, scrollable,
        space, stack, text, text::Wrapping, Button,
    },
    Alignment, Color, Element, Function, Length, Subscription, Task, Theme,
};

use prism::{axis, line_series, Axis, Chart, Labels};

use std::{
    fmt::Display,
    path::{Path, PathBuf},
    sync::Arc,
};

pub struct Main {
    state: State,
    selected: Option<measurement::Selected>,
    signal_cache: canvas::Cache,
    smoothing: frequency_response::Smoothing,
    loopback: Option<ui::Loopback>,
    measurements: Vec<ui::measurement::State>,
    modal: Modal,
    zoom: chart::Zoom,
    offset: chart::Offset,
    project_path: Option<PathBuf>,
}

#[allow(clippy::large_enum_variant)]
enum State {
    CollectingMeasuremnts {
        recording: Option<Recording>,
    },
    Analysing {
        active_tab: Tab,
        window: Window<Samples>,
        selected_impulse_response: Option<ui::measurement::Id>,
        charts: Charts,
    },
}

impl Default for State {
    fn default() -> Self {
        State::CollectingMeasuremnts { recording: None }
    }
}

#[allow(clippy::large_enum_variant)]
pub enum Tab {
    Measurements { recording: Option<Recording> },
    ImpulseResponses { window_settings: WindowSettings },
    FrequencyResponses,
    SpectralDecay,
    Spectrogram,
}

impl Default for Tab {
    fn default() -> Self {
        Self::Measurements { recording: None }
    }
}

#[derive(Debug, Default)]
struct Charts {
    impulse_responses: impulse_response::Chart,
    frequency_responses: frequency_response::ChartData,
    spectral_decay_cache: canvas::Cache,
    spectrogram: Spectrogram,
}

#[derive(Debug, Default)]
struct Spectrogram {
    pub zoom: chart::Zoom,
    pub offset: chart::Offset,
    pub cache: canvas::Cache,
}

#[derive(Default, Debug)]
enum Modal {
    #[default]
    None,
    PendingWindow {
        goto_tab: TabId,
    },
    // ReplaceLoopback {
    //     loopback: data::measurement::State<data::measurement::Loopback>,
    // },
}

#[derive(Debug, Clone)]
pub enum ModalAction {
    Discard,
    Apply,
}

#[derive(Debug, Clone)]
pub enum Message {
    NewProject,
    LoadProject,
    SaveProject,
    RecentProject(usize),
    ProjectLoaded(Result<(Arc<data::Project>, PathBuf), PickAndLoadError>),
    ProjectSaved(Result<PathBuf, PickAndSaveError>),

    TabSelected(TabId),

    Measurements(measurement::Message),
    MeasurementChart(waveform::Interaction),
    ImpulseResponseSelected(ui::measurement::Id),

    FrequencyResponseToggled(ui::measurement::Id, bool),
    SmoothingChanged(frequency_response::Smoothing),
    FrequencyResponseSmoothed((ui::measurement::Id, Box<[f32]>)),
    FrequencyResponseComputed(ui::measurement::Id, data::FrequencyResponse),

    ImpulseResponses(impulse_response::Message),
    SaveImpulseResponseFileDialog(ui::measurement::Id),
    SaveImpulseResponse(ui::measurement::Id, Arc<Path>),
    ImpulseResponsesSaved(Arc<Path>),

    ShiftKeyPressed,
    ShiftKeyReleased,

    ImpulseResponseComputed(ui::measurement::Id, data::ImpulseResponse),

    Recording(recording::Message),
    StartRecording(recording::Kind),

    SpectralDecayComputed(ui::measurement::Id, data::SpectralDecay),

    Spectrogram(chart::spectrogram::Interaction),
    SpectrogramComputed(ui::measurement::Id, data::Spectrogram),

    Modal(ModalAction),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabId {
    Measurements,
    ImpulseResponses,
    FrequencyResponses,
    SpectralDecay,
    Spectrogram,
}

impl Main {
    pub fn from_project(path: PathBuf, project: data::Project) -> (Self, Task<Message>) {
        let load_loopback = project
            .loopback
            .map(|loopback| {
                Task::perform(
                    measurement::load_measurement(loopback.0.path, measurement::Kind::Loopback),
                    measurement::Message::Loaded,
                )
            })
            .unwrap_or(Task::none());

        let load_measurements = project.measurements.into_iter().map(|measurement| {
            Task::perform(
                measurement::load_measurement(measurement.path, measurement::Kind::Normal),
                measurement::Message::Loaded,
            )
        });

        (
            Self {
                project_path: Some(path),
                ..Default::default()
            },
            Task::batch([load_loopback, Task::batch(load_measurements)]).map(Message::Measurements),
        )
    }

    pub fn update(
        &mut self,
        recent_projects: &mut RecentProjects,
        message: Message,
    ) -> Task<Message> {
        match message {
            Message::TabSelected(id) => {
                let State::Analysing {
                    ref mut active_tab,
                    ref window,
                    ..
                } = self.state
                else {
                    return Task::none();
                };

                let (tab, tasks) = match (&active_tab, id) {
                    (Tab::Measurements { .. }, TabId::Measurements)
                    | (Tab::ImpulseResponses { .. }, TabId::ImpulseResponses)
                    | (Tab::FrequencyResponses, TabId::FrequencyResponses) => return Task::none(),
                    (
                        Tab::ImpulseResponses {
                            ref window_settings,
                            ..
                        },
                        tab_id,
                    ) if window_settings.window != *window => {
                        self.modal = Modal::PendingWindow { goto_tab: tab_id };
                        return Task::none();
                    }
                    (_, TabId::Measurements) => {
                        (Tab::Measurements { recording: None }, Task::none())
                    }
                    (_, TabId::ImpulseResponses) => (
                        Tab::ImpulseResponses {
                            window_settings: WindowSettings::new(window.clone()),
                        },
                        Task::none(),
                    ),
                    (_, TabId::FrequencyResponses) => {
                        let tasks = self
                            .measurements
                            .iter_mut()
                            .flat_map(ui::measurement::State::loaded_mut)
                            .map(|measurement| {
                                compute_frequency_response(
                                    self.loopback.as_ref().unwrap(),
                                    measurement,
                                    window,
                                )
                            });
                        (Tab::FrequencyResponses, Task::batch(tasks))
                    }
                    (_, TabId::SpectralDecay) => (Tab::SpectralDecay, Task::none()),
                    (_, TabId::Spectrogram) => (Tab::Spectrogram, Task::none()),
                };

                *active_tab = tab;

                tasks
            }
            Message::Measurements(message) => {
                let task = match message {
                    measurement::Message::Load(kind) => {
                        let dialog_caption = kind.to_string();

                        Task::perform(
                            measurement::pick_file_and_load_signal(dialog_caption, kind),
                            measurement::Message::Loaded,
                        )
                    }
                    measurement::Message::Loaded(Ok(result)) => {
                        match Arc::into_inner(result) {
                            Some(measurement::LoadedKind::Loopback(loopback)) => {
                                self.loopback = Some(ui::Loopback::from_data(loopback))
                            }
                            Some(measurement::LoadedKind::Normal(measurement)) => self
                                .measurements
                                .push(ui::measurement::State::from_data(measurement)),
                            None => {}
                        }

                        Task::none()
                    }
                    measurement::Message::Loaded(Err(err)) => {
                        log::error!("{err}");
                        Task::none()
                    }
                    measurement::Message::Remove(index) => {
                        self.measurements.remove(index);
                        Task::none()
                    }
                    measurement::Message::Select(selected) => {
                        self.selected = Some(selected);
                        self.signal_cache.clear();
                        Task::none()
                    }
                };

                let state = std::mem::take(&mut self.state);
                self.state = match (state, self.analysing_possible()) {
                    (State::CollectingMeasuremnts { recording }, false) => {
                        State::CollectingMeasuremnts { recording }
                    }
                    (State::CollectingMeasuremnts { recording }, true) => State::Analysing {
                        active_tab: Tab::Measurements { recording },
                        window: Window::new(SampleRate::from(
                            self.loopback
                                .as_ref()
                                .and_then(ui::Loopback::loaded)
                                .map_or(44_100, |l| l.as_ref().sample_rate()),
                        ))
                        .into(),
                        selected_impulse_response: None,
                        charts: Charts::default(),
                    },
                    (old_state, true) => old_state,
                    (State::Analysing { .. }, false) => {
                        State::CollectingMeasuremnts { recording: None }
                    }
                };

                task.map(Message::Measurements)
            }
            Message::ImpulseResponseSelected(id) => {
                log::debug!("Impulse response selected: {id}");

                let State::Analysing {
                    ref mut selected_impulse_response,
                    ref mut charts,
                    ref active_tab,
                    ..
                } = self.state
                else {
                    return Task::none();
                };

                *selected_impulse_response = Some(id);
                charts.impulse_responses.data_cache.clear();

                let Some(measurement) = self
                    .measurements
                    .iter_mut()
                    .filter_map(ui::measurement::State::loaded_mut)
                    .find(|m| m.id == id)
                else {
                    return Task::none();
                };

                match active_tab {
                    Tab::Measurements { .. } => Task::none(),
                    Tab::ImpulseResponses { .. } => {
                        compute_impulse_response(self.loopback.as_ref().unwrap(), measurement)
                    }
                    Tab::FrequencyResponses => Task::none(),
                    Tab::SpectralDecay => {
                        compute_spectral_decay(self.loopback.as_ref().unwrap(), measurement)
                    }
                    Tab::Spectrogram => {
                        compute_spectrogram(self.loopback.as_ref().unwrap(), measurement)
                    }
                }
            }
            Message::ImpulseResponseComputed(id, impulse_response) => {
                let State::Analysing {
                    window,
                    active_tab,
                    selected_impulse_response,
                    charts,
                    ..
                } = &mut self.state
                else {
                    return Task::none();
                };

                let impulse_response = ui::ImpulseResponse::from_data(impulse_response);

                let Some(measurement) = self
                    .measurements
                    .iter_mut()
                    .filter_map(ui::measurement::State::loaded_mut)
                    .find(|m| m.id == id)
                else {
                    return Task::none();
                };

                measurement
                    .analysis
                    .impulse_response
                    .computed(impulse_response.clone());

                if selected_impulse_response.is_some_and(|selected| selected == id) {
                    charts
                        .impulse_responses
                        .x_range
                        .get_or_insert(0.0..=impulse_response.data.len() as f32);

                    charts.impulse_responses.data_cache.clear();
                }

                if let Tab::FrequencyResponses = active_tab {
                    compute_frequency_response(self.loopback.as_ref().unwrap(), measurement, window)
                } else if let Tab::SpectralDecay = active_tab {
                    compute_spectral_decay(self.loopback.as_ref().unwrap(), measurement)
                } else if let Tab::Spectrogram = active_tab {
                    compute_spectrogram(self.loopback.as_ref().unwrap(), measurement)
                } else {
                    Task::none()
                }
            }
            Message::ImpulseResponses(impulse_response::Message::Chart(operation)) => {
                let State::Analysing {
                    active_tab:
                        Tab::ImpulseResponses {
                            ref mut window_settings,
                            ..
                        },
                    ref mut charts,
                    ..
                } = self.state
                else {
                    return Task::none();
                };

                if let ChartOperation::Interaction(ref interaction) = operation {
                    match interaction {
                        chart::Interaction::HandleMoved(index, new_pos) => {
                            let mut handles: window::Handles = Into::into(&window_settings.window);
                            handles.update(*index, *new_pos);
                            window_settings.window.update(handles);
                        }
                        chart::Interaction::ZoomChanged(zoom) => {
                            charts.impulse_responses.zoom = *zoom;
                        }
                        chart::Interaction::OffsetChanged(offset) => {
                            charts.impulse_responses.offset = *offset;
                        }
                    }
                }

                charts.impulse_responses.update(operation);

                Task::none()
            }
            Message::FrequencyResponseComputed(id, frequency_response) => {
                log::debug!("Frequency response computed: {id}");

                let State::Analysing { ref mut charts, .. } = self.state else {
                    return Task::none();
                };

                let Some(measurement) = self
                    .measurements
                    .iter_mut()
                    .filter_map(ui::measurement::State::loaded_mut)
                    .find(|m| m.id == id)
                else {
                    return Task::none();
                };

                measurement
                    .analysis
                    .frequency_response
                    .computed(frequency_response.clone());

                charts.frequency_responses.cache.clear();

                if let Some(fraction) = self.smoothing.fraction() {
                    Task::perform(
                        frequency_response::smooth_frequency_response(
                            id,
                            frequency_response,
                            fraction,
                        ),
                        Message::FrequencyResponseSmoothed,
                    )
                } else {
                    Task::none()
                }
            }
            Message::FrequencyResponseToggled(id, state) => {
                let State::Analysing { ref mut charts, .. } = self.state else {
                    return Task::none();
                };

                let Some(frequency_response) = self
                    .measurements
                    .iter_mut()
                    .filter_map(ui::measurement::State::loaded_mut)
                    .find(|m| m.id == id)
                    .map(|m| &mut m.analysis.frequency_response)
                else {
                    return Task::none();
                };

                frequency_response.is_shown = state;

                charts.frequency_responses.cache.clear();

                Task::none()
            }
            Message::SmoothingChanged(smoothing) => {
                let State::Analysing { ref mut charts, .. } = self.state else {
                    return Task::none();
                };

                self.smoothing = smoothing;

                if let Some(fraction) = smoothing.fraction() {
                    let tasks = self
                        .measurements
                        .iter()
                        .filter_map(ui::measurement::State::loaded)
                        .flat_map(|m| {
                            m.analysis
                                .frequency_response
                                .result()
                                .cloned()
                                .map(|fr| (m.id, fr))
                        })
                        .map(|(id, fr)| {
                            Task::perform(
                                frequency_response::smooth_frequency_response(id, fr, fraction),
                                Message::FrequencyResponseSmoothed,
                            )
                        });

                    Task::batch(tasks)
                } else {
                    self.measurements
                        .iter_mut()
                        .filter_map(ui::measurement::State::loaded_mut)
                        .map(|m| &mut m.analysis.frequency_response)
                        .for_each(|fr| fr.smoothed = None);

                    charts.frequency_responses.cache.clear();

                    Task::none()
                }
            }
            Message::FrequencyResponseSmoothed((id, smoothed_data)) => {
                let State::Analysing { ref mut charts, .. } = self.state else {
                    return Task::none();
                };

                let Some(frequency_response) = self
                    .measurements
                    .iter_mut()
                    .filter_map(ui::measurement::State::loaded_mut)
                    .find(|m| m.id == id)
                    .map(|m| &mut m.analysis.frequency_response)
                else {
                    return Task::none();
                };

                frequency_response.smoothed = Some(smoothed_data);
                charts.frequency_responses.cache.clear();

                Task::none()
            }
            Message::ShiftKeyPressed => {
                let State::Analysing { ref mut charts, .. } = self.state else {
                    return Task::none();
                };

                charts.impulse_responses.shift_key_pressed();

                Task::none()
            }
            Message::ShiftKeyReleased => {
                let State::Analysing { ref mut charts, .. } = self.state else {
                    return Task::none();
                };

                charts.impulse_responses.shift_key_released();

                Task::none()
            }
            Message::Modal(action) => {
                let Modal::PendingWindow { goto_tab } = std::mem::take(&mut self.modal) else {
                    return Task::none();
                };

                let State::Analysing {
                    ref mut active_tab,
                    ref mut window,
                    ..
                } = self.state
                else {
                    return Task::none();
                };

                let Tab::ImpulseResponses {
                    ref mut window_settings,
                    ..
                } = active_tab
                else {
                    return Task::none();
                };

                match action {
                    ModalAction::Discard => {
                        *window_settings = WindowSettings::new(window.clone());
                    }
                    ModalAction::Apply => {
                        self.measurements
                            .iter_mut()
                            .filter_map(ui::measurement::State::loaded_mut)
                            .for_each(|m| {
                                m.analysis.frequency_response = ui::FrequencyResponse::default();
                                m.analysis.spectral_decay = ui::spectral_decay::State::default();
                                m.analysis.spectrogram = ui::spectrogram::State::default();
                            });

                        *window = window_settings.window.clone();
                    }
                }

                Task::done(Message::TabSelected(goto_tab))
            }
            Message::Recording(message) => {
                let (State::CollectingMeasuremnts { ref mut recording }
                | State::Analysing {
                    active_tab: Tab::Measurements { ref mut recording },
                    ..
                }) = self.state
                else {
                    return Task::none();
                };

                if let Some(view) = recording {
                    match view.update(message) {
                        recording::Action::None => Task::none(),
                        recording::Action::Cancel => {
                            *recording = None;
                            Task::none()
                        }
                        recording::Action::Finished(result) => {
                            match result {
                                recording::Result::Loopback(loopback) => {
                                    self.loopback =
                                        Some(ui::Loopback::new("Loopback".to_string(), loopback))
                                }
                                recording::Result::Measurement(measurement) => {
                                    self.measurements.push(ui::measurement::State::new(
                                        "Measurement".to_string(),
                                        measurement,
                                    ))
                                }
                            }
                            *recording = None;
                            Task::none()
                        }
                        recording::Action::Task(task) => task.map(Message::Recording),
                    }
                } else {
                    Task::none()
                }
            }
            Message::StartRecording(kind) => match &mut self.state {
                State::CollectingMeasuremnts { recording }
                | State::Analysing {
                    active_tab: Tab::Measurements { recording },
                    ..
                } => {
                    *recording = Some(Recording::new(kind));
                    Task::none()
                }
                _ => Task::none(),
            },
            Message::MeasurementChart(interaction) => {
                match interaction {
                    waveform::Interaction::ZoomChanged(zoom) => self.zoom = zoom,
                    waveform::Interaction::OffsetChanged(offset) => self.offset = offset,
                }

                self.signal_cache.clear();

                Task::none()
            }
            Message::SaveImpulseResponseFileDialog(id) => {
                Task::future(choose_impulse_response_file_path())
                    .and_then(Task::done)
                    .map(Message::SaveImpulseResponse.with(id))
            }
            Message::SaveImpulseResponse(id, path) => {
                let State::Analysing {
                    active_tab: Tab::ImpulseResponses { .. },
                    ..
                } = &self.state
                else {
                    return Task::none();
                };

                if let Some(impulse_response) = self
                    .measurements
                    .iter()
                    .filter_map(ui::measurement::State::loaded)
                    .find(|m| m.id == id)
                    .and_then(|m| m.analysis.impulse_response.result())
                    .cloned()
                {
                    Task::perform(
                        save_impulse_response(path.clone(), impulse_response),
                        |_| Message::ImpulseResponsesSaved(path),
                    )
                } else {
                    self.compute_impulse_response(id)
                        .chain(Task::done(Message::SaveImpulseResponse(id, path)))
                }
            }
            Message::ImpulseResponsesSaved(path) => {
                log::debug!("Impulse response saved to: {}", path.display());

                Task::none()
            }
            Message::NewProject => {
                *self = Self::default();

                Task::none()
            }
            Message::LoadProject => Task::perform(
                crate::pick_project_file().then(async |res| {
                    let path = res?;
                    load_project(path).await
                }),
                Message::ProjectLoaded,
            ),
            Message::ProjectLoaded(Ok((project, path))) => match Arc::into_inner(project) {
                Some(project) => {
                    recent_projects.insert(path.clone());

                    let (screen, tasks) = Self::from_project(path, project);
                    *self = screen;

                    Task::batch([
                        tasks,
                        Task::future(recent_projects.clone().save()).discard(),
                    ])
                }
                None => Task::none(),
            },
            Message::ProjectLoaded(Err(err)) => {
                log::debug!("Loading project failed: {err}");

                Task::none()
            }
            Message::RecentProject(id) => match recent_projects.get(id) {
                Some(path) => Task::perform(load_project(path.clone()), Message::ProjectLoaded),
                None => Task::none(),
            },
            Message::SaveProject => {
                let loopback = self
                    .loopback
                    .as_ref()
                    .and_then(|l| l.path.clone())
                    .map(|path| project::Loopback(project::Measurement { path }));

                let measurements = self
                    .measurements
                    .iter()
                    .flat_map(ui::measurement::State::loaded)
                    .flat_map(|m| m.path.clone())
                    .map(|path| project::Measurement { path })
                    .collect();

                let project = Project {
                    loopback,
                    measurements,
                };

                if let Some(path) = self.project_path.clone() {
                    Task::perform(
                        project
                            .save(path.clone())
                            .map_ok(move |_| path)
                            .map_err(PickAndSaveError::File),
                        Message::ProjectSaved,
                    )
                } else {
                    Task::perform(
                        pick_project_file().then(async |res| {
                            let path = res?;
                            project.save(path.clone()).await?;

                            Ok(path)
                        }),
                        Message::ProjectSaved,
                    )
                }
            }
            Message::ProjectSaved(Ok(path)) => {
                log::debug!("Project saved.");

                self.project_path = Some(path);

                Task::none()
            }
            Message::ProjectSaved(Err(err)) => {
                log::debug!("Saving project failed: {err}");
                Task::none()
            }
            Message::SpectralDecayComputed(id, decay) => {
                log::debug!(
                    "Spectral decay for measurement (ID: {}) with: {} slices, computed.",
                    id,
                    decay.len()
                );

                let Some(spectral_decay) = self
                    .measurements
                    .iter_mut()
                    .filter_map(ui::measurement::State::loaded_mut)
                    .find(|m| m.id == id)
                    .map(|m| &mut m.analysis.spectral_decay)
                else {
                    return Task::none();
                };

                spectral_decay.computed(decay);

                if let State::Analysing { charts, .. } = &mut self.state {
                    charts.spectral_decay_cache.clear();
                };

                Task::none()
            }
            Message::Spectrogram(interaction) => {
                log::debug!("Spectrogram chart: {interaction:?}.");

                let State::Analysing { charts, .. } = &mut self.state else {
                    return Task::none();
                };

                match interaction {
                    chart::spectrogram::Interaction::ZoomChanged(zoom) => {
                        charts.spectrogram.zoom = zoom
                    }
                    chart::spectrogram::Interaction::OffsetChanged(offset) => {
                        charts.spectrogram.offset = offset
                    }
                }

                charts.spectrogram.cache.clear();

                Task::none()
            }
            Message::SpectrogramComputed(id, data) => {
                log::debug!(
                    "Spectrogram for measurement (ID: {}) with: {} slices, computed.",
                    id,
                    data.len()
                );

                let Some(spectrogram) = self
                    .measurements
                    .iter_mut()
                    .filter_map(ui::measurement::State::loaded_mut)
                    .find(|m| m.id == id)
                    .map(|m| &mut m.analysis.spectrogram)
                else {
                    return Task::none();
                };

                spectrogram.computed(data);

                if let State::Analysing { charts, .. } = &mut self.state {
                    charts.spectrogram.cache.clear();
                };

                Task::none()
            }
        }
    }

    pub fn view<'a>(&'a self, recent_projects: &'a RecentProjects) -> Element<'a, Message> {
        let recent_project_entries = column(
            recent_projects
                .iter()
                .enumerate()
                .filter_map(|(i, p)| p.file_name().map(|f| (i, f)))
                .filter_map(|(i, p)| p.to_str().map(|f| (i, f)))
                .map(|(i, s)| {
                    button(s)
                        .on_press(Message::RecentProject(i))
                        .style(button::subtle)
                        .width(Length::Fill)
                        .into()
                }),
        )
        .width(Length::Fill);

        let project_menu = column![
            button("New")
                .on_press(Message::NewProject)
                .style(button::subtle)
                .width(Length::Fill),
            button("Save")
                .on_press(Message::SaveProject)
                .style(button::subtle)
                .width(Length::Fill),
            button("Open ...")
                .on_press(Message::LoadProject)
                .style(button::subtle)
                .width(Length::Fill),
            dropdown_menu("Open recent ...", recent_project_entries)
                .style(button::subtle)
                .width(Length::Fill),
        ]
        .width(Length::Fill);

        let header = container(column![
            dropdown_root("Project", project_menu).style(button::secondary),
            match &self.state {
                State::CollectingMeasuremnts { .. } => TabId::Measurements.view(false),
                State::Analysing { active_tab, .. } => TabId::from(active_tab).view(true),
            }
        ])
        .width(Length::Fill)
        .style(container::dark);

        let content = match &self.state {
            State::CollectingMeasuremnts { recording } => {
                if let Some(recording) = recording {
                    recording.view().map(Message::Recording)
                } else {
                    self.measurements_tab()
                }
            }
            State::Analysing {
                active_tab,
                selected_impulse_response,
                charts,
                ..
            } => match active_tab {
                Tab::Measurements { recording } => {
                    if let Some(recording) = recording {
                        recording.view().map(Message::Recording)
                    } else {
                        self.measurements_tab()
                    }
                }
                Tab::ImpulseResponses {
                    window_settings, ..
                } => self.impulse_responses_tab(
                    *selected_impulse_response,
                    &charts.impulse_responses,
                    window_settings,
                ),
                Tab::FrequencyResponses => {
                    self.frequency_responses_tab(&charts.frequency_responses)
                }
                Tab::SpectralDecay => self
                    .spectral_decay_tab(*selected_impulse_response, &charts.spectral_decay_cache),
                Tab::Spectrogram => {
                    self.spectrogram_tab(*selected_impulse_response, &charts.spectrogram)
                }
            },
        };

        let content = container(column![header, container(content).padding(10)]);

        if let Modal::PendingWindow { .. } = self.modal {
            let pending_window = {
                container(
                    column![
                        text("Window pending!").size(18),
                        column![
                            text("You have modified the window used for frequency response computations."),
                            text("You need to discard or apply your changes before proceeding."),
                        ].spacing(5),
                        row![
                            space::horizontal(),
                            button("Discard")
                                .style(button::danger)
                                .on_press(Message::Modal(ModalAction::Discard)),
                            button("Apply")
                                .style(button::success)
                                .on_press(Message::Modal(ModalAction::Apply))
                        ]
                        .spacing(5)
                    ]
                    .spacing(10))
                    .padding(20)
                    .width(400)
                    .style(container::bordered_box)
            };

            modal(content, pending_window)
        } else {
            content.into()
        }
    }

    fn measurements_tab(&self) -> Element<'_, Message> {
        let sidebar = {
            let loopback = Category::new("Loopback")
                .push_button(
                    button("+")
                        .on_press_maybe(Some(Message::Measurements(measurement::Message::Load(
                            measurement::Kind::Loopback,
                        ))))
                        .style(button::secondary),
                )
                .push_button(
                    button(icon::record())
                        .on_press(Message::StartRecording(recording::Kind::Loopback))
                        .style(button::secondary),
                )
                .push_entry_maybe(self.loopback.as_ref().map(|loopback| {
                    measurement::loopback_entry(self.selected, loopback).map(Message::Measurements)
                }));

            let measurements =
                Category::new("Measurements")
                    .push_button(button("+").style(button::secondary).on_press(
                        Message::Measurements(measurement::Message::Load(
                            measurement::Kind::Normal,
                        )),
                    ))
                    .push_button(
                        button(icon::record())
                            .on_press(Message::StartRecording(recording::Kind::Measurement))
                            .style(button::secondary),
                    )
                    .extend_entries(self.measurements.iter().enumerate().map(
                        |(id, measurement)| {
                            measurement::list_entry(id, self.selected, measurement)
                                .map(Message::Measurements)
                        },
                    ));

            container(scrollable(
                column![loopback, measurements].spacing(20).padding(10),
            ))
            .style(container::rounded_box)
        };

        let content: Element<_> = {
            let welcome_text = |base_text| -> Element<Message> {
                column![
                    text("Welcome").size(24),
                    column![
                        base_text,
                        row![
                            text("You can load signals from file by pressing [+] or"),
                            button(icon::record()).style(button::secondary) // .on_press(Message::StartRecording(recording::Kind::Measurement))
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

            let content = if let Some(measurement) =
                self.selected.and_then(|selected| match selected {
                    measurement::Selected::Loopback => self
                        .loopback
                        .as_ref()
                        .and_then(|l| l.loaded())
                        .map(AsRef::as_ref),
                    measurement::Selected::Measurement(i) => self
                        .measurements
                        .get(i)
                        .and_then(ui::measurement::State::loaded)
                        .map(|m| &m.data),
                }) {
                chart::waveform(measurement, &self.signal_cache, self.zoom, self.offset)
                    .map(Message::MeasurementChart)
            } else {
                welcome_text(text("Select a signal to view its data."))
            };

            container(content).center(Length::Fill).into()
        };

        column!(row![
            container(sidebar).width(Length::FillPortion(1)),
            container(content).width(Length::FillPortion(4))
        ])
        .spacing(10)
        .into()
    }

    pub fn impulse_responses_tab<'a>(
        &'a self,
        selected: Option<ui::measurement::Id>,
        chart: &'a impulse_response::Chart,
        window_settings: &'a WindowSettings,
    ) -> Element<'a, Message> {
        let sidebar = {
            let header = sidebar::header("Impulse Responses");

            let entries = self
                .measurements
                .iter()
                .filter_map(ui::measurement::State::loaded)
                .map(|measurement| impulse_response_item(selected, measurement));

            container(column![header, scrollable(column(entries))].spacing(6))
                .padding(6)
                .style(|theme| {
                    container::rounded_box(theme)
                        .background(theme.extended_palette().background.weakest.color)
                })
        };

        let content = {
            let placeholder = container(text("Impulse response not computed, yet.")).into();

            self.measurements
                .iter()
                .filter_map(ui::measurement::State::loaded)
                .find(|m| Some(m.id) == selected)
                .and_then(|measurement| measurement.analysis.impulse_response.result())
                .map_or(placeholder, |impulse_response| {
                    chart
                        .view(impulse_response, window_settings)
                        .map(Message::ImpulseResponses)
                })
        };

        row![
            container(sidebar).width(Length::FillPortion(2)),
            container(content).center(Length::FillPortion(5))
        ]
        .spacing(10)
        .into()
    }

    fn frequency_responses_tab<'a>(
        &'a self,
        chart_settings: &'a frequency_response::ChartData,
    ) -> Element<'a, Message> {
        let loaded_measurements = self
            .measurements
            .iter()
            .filter_map(ui::measurement::State::loaded);

        let sidebar = {
            let entries = loaded_measurements.clone().map(|measurement| {
                let name = &measurement.name;
                measurement.analysis.frequency_response.view(
                    name,
                    measurement.analysis.impulse_response.progress(),
                    Message::FrequencyResponseToggled.with(measurement.id),
                )
            });

            container(column(entries).spacing(10).padding(8)).style(container::rounded_box)
        };

        let header = {
            row![pick_list(
                frequency_response::Smoothing::ALL,
                Some(&self.smoothing),
                Message::SmoothingChanged,
            )]
        };

        let enabled_frequency_responses =
            loaded_measurements.map(|m| &m.analysis.frequency_response);

        let content = if enabled_frequency_responses
            .clone()
            .any(|fr| fr.is_shown && fr.result().is_some())
        {
            let series_list = enabled_frequency_responses
                .flat_map(|item| {
                    let Some(frequency_response) = item.result() else {
                        return [None, None];
                    };

                    let sample_rate = frequency_response.sample_rate;
                    let len = frequency_response.data.len() * 2 + 1;
                    let resolution = sample_rate as f32 / len as f32;

                    let closure = move |(i, s)| (i as f32 * resolution, dbfs(s));

                    [
                        Some(
                            line_series(
                                frequency_response
                                    .data
                                    .iter()
                                    .copied()
                                    .enumerate()
                                    .map(closure),
                            )
                            .color(item.color.scale_alpha(0.1)),
                        ),
                        item.smoothed.as_ref().map(|smoothed| {
                            line_series(smoothed.iter().copied().enumerate().map(closure))
                                .color(item.color)
                        }),
                    ]
                })
                .flatten();

            let chart: Chart<Message, ()> = Chart::new()
                .x_axis(
                    Axis::new(axis::Alignment::Horizontal)
                        .scale(axis::Scale::Log)
                        .x_tick_marks(
                            [20, 50, 100, 1000, 10_000, 20_000]
                                .into_iter()
                                .map(|v| v as f32)
                                .collect(),
                        ),
                )
                .x_range(chart_settings.x_range.clone().unwrap_or(20.0..=22_500.0))
                .y_labels(Labels::default().format(&|v| format!("{v:.0}")))
                .extend_series(series_list)
                .cache(&chart_settings.cache);
            // .on_scroll(|state| {
            //     let pos = state.get_coords();
            //     let delta = state.scroll_delta();
            //     let x_range = state.x_range();
            //     Message::Chart(ChartOperation::Scroll(pos, delta, x_range))
            // });

            container(chart)
        } else {
            container(text("Please select a frequency respone."))
        };

        row![
            container(sidebar)
                .width(Length::FillPortion(3))
                .style(container::bordered_box),
            column![header, container(content).width(Length::FillPortion(10))].spacing(12)
        ]
        .spacing(10)
        .into()
    }

    pub fn spectral_decay_tab<'a>(
        &'a self,
        selected: Option<ui::measurement::Id>,
        cache: &'a canvas::Cache,
    ) -> Element<'a, Message> {
        let sidebar = {
            let header = {
                column!(text("For Measurements"), rule::horizontal(1))
                    .width(Length::Fill)
                    .spacing(5)
            };

            let entries = self
                .measurements
                .iter()
                .filter_map(ui::measurement::State::loaded)
                .map(|measurement| {
                    let id = measurement.id;

                    let entry = {
                        let btn = button(
                            column![
                                text(&measurement.name)
                                    .size(16)
                                    .wrapping(Wrapping::WordOrGlyph),
                                text("10.12.2019 10:24:12").size(10)
                            ]
                            .clip(true)
                            .padding(3)
                            .spacing(6),
                        )
                        // FIXME: rename message
                        .on_press_with(move || Message::ImpulseResponseSelected(id))
                        .width(Length::Fill)
                        .style(move |theme, status| {
                            let status = match selected {
                                Some(selected) if selected == id => button::Status::Hovered,
                                _ => status,
                            };
                            button::secondary(theme, status)
                        });

                        container(btn).style(container::dark).padding(6).into()
                    };

                    match measurement.analysis.spectral_decay_progress() {
                        ui::spectral_decay::Progress::None => entry,
                        ui::spectral_decay::Progress::ComputingImpulseResponse => {
                            processing_overlay("Impulse Response", entry)
                        }
                        ui::spectral_decay::Progress::Computing => {
                            processing_overlay("Spectral Decay", entry)
                        }
                        ui::spectral_decay::Progress::Finished => entry,
                    }
                });

            container(scrollable(
                column![header, column(entries).spacing(3)]
                    .spacing(10)
                    .padding(10),
            ))
            .style(container::rounded_box)
        };

        let content = if let Some(decay) = self
            .measurements
            .iter()
            .filter_map(ui::measurement::State::loaded)
            .find(|m| Some(m.id) == selected)
            .and_then(|m| m.analysis.spectral_decay.result())
        {
            let gradient = colorous::MAGMA;

            let series_list = decay.iter().enumerate().map(|(fr_index, fr)| {
                let sample_rate = fr.sample_rate;
                let len = fr.data.len() * 2 + 1;
                let resolution = sample_rate as f32 / len as f32;

                let closure = move |(i, s)| (i as f32 * resolution, dbfs(s));

                let color = gradient.eval_rational(fr_index, decay.len());
                line_series(fr.data.iter().copied().enumerate().map(closure))
                    .color(iced::Color::from_rgb8(color.r, color.g, color.b))
            });

            let chart: Chart<Message, ()> = Chart::new()
                .x_axis(
                    Axis::new(axis::Alignment::Horizontal)
                        .scale(axis::Scale::Log)
                        .x_tick_marks(
                            [10, 20, 50, 100, 1000]
                                .into_iter()
                                .map(|v| v as f32)
                                .collect(),
                        ),
                )
                // .x_range(20.0..=2000.0)
                .y_labels(Labels::default().format(&|v| format!("{v:.0}")))
                .extend_series(series_list)
                .cache(cache);

            container(chart)
        } else {
            container(text("Please select a frequency respone."))
        };

        row![
            container(sidebar)
                .width(Length::FillPortion(3))
                .style(container::bordered_box),
            column![container(content).width(Length::FillPortion(10))].spacing(12)
        ]
        .spacing(10)
        .into()
    }

    fn spectrogram_tab<'a>(
        &'a self,
        selected: Option<ui::measurement::Id>,
        spectrogram: &'a Spectrogram,
    ) -> Element<'a, Message> {
        let sidebar = {
            let header = {
                column!(text("For Measurements"), rule::horizontal(1))
                    .width(Length::Fill)
                    .spacing(5)
            };

            let entries = self
                .measurements
                .iter()
                .filter_map(ui::measurement::State::loaded)
                .map(|measurement| {
                    let id = measurement.id;

                    let entry = {
                        let btn = button(
                            column![
                                text(&measurement.name)
                                    .size(16)
                                    .wrapping(Wrapping::WordOrGlyph),
                                text("10.12.2019 10:24:12").size(10)
                            ]
                            .clip(true)
                            .padding(3)
                            .spacing(6),
                        )
                        // FIXME: rename message
                        .on_press_with(move || Message::ImpulseResponseSelected(id))
                        .width(Length::Fill)
                        .style(move |theme, status| {
                            let status = match selected {
                                Some(selected) if selected == id => button::Status::Hovered,
                                _ => status,
                            };
                            button::secondary(theme, status)
                        });

                        container(btn).style(container::dark).padding(6).into()
                    };

                    match measurement.analysis.spectrogram_progress() {
                        ui::spectrogram::Progress::None => entry,
                        ui::spectrogram::Progress::ComputingImpulseResponse => {
                            processing_overlay("Impulse Response", entry)
                        }
                        ui::spectrogram::Progress::Computing => {
                            processing_overlay("Spectral Decay", entry)
                        }
                        ui::spectrogram::Progress::Finished => entry,
                    }
                });

            container(scrollable(
                column![header, column(entries).spacing(3)]
                    .spacing(10)
                    .padding(10),
            ))
            .style(container::rounded_box)
        };

        let content = if let Some(data) = self
            .measurements
            .iter()
            .filter_map(ui::measurement::State::loaded)
            .find(|m| Some(m.id) == selected)
            .and_then(|m| m.analysis.spectrogram.result())
        {
            let chart = chart::spectrogram(
                data,
                &spectrogram.cache,
                spectrogram.zoom,
                spectrogram.offset,
            )
            .map(Message::Spectrogram);

            container(chart)
        } else {
            container(text("Please select a frequency respone."))
        };

        row![
            container(sidebar)
                .width(Length::FillPortion(3))
                .style(container::bordered_box),
            column![container(content).width(Length::FillPortion(10))].spacing(12)
        ]
        .spacing(10)
        .into()
    }

    pub fn subscription(&self) -> Subscription<Message> {
        let hotkeys_pressed = keyboard::on_key_press(|key, _modifiers| {
            use keyboard::key::{Key, Named};

            Some(match key.as_ref() {
                Key::Named(Named::Shift) => Message::ShiftKeyPressed,
                _ => None?,
            })
        });

        let hotkeys_released = keyboard::on_key_release(|key, _modifiers| {
            use keyboard::key::{Key, Named};

            Some(match key.as_ref() {
                Key::Named(Named::Shift) => Message::ShiftKeyReleased,
                _ => None?,
            })
        });

        let recording = match &self.state {
            State::CollectingMeasuremnts {
                recording: Some(recording),
            }
            | State::Analysing {
                active_tab:
                    Tab::Measurements {
                        recording: Some(recording),
                    },
                ..
            } => recording.subscription().map(Message::Recording),
            _ => Subscription::none(),
        };

        Subscription::batch([hotkeys_pressed, hotkeys_released, recording])
    }

    fn analysing_possible(&self) -> bool {
        self.loopback.as_ref().is_some_and(ui::Loopback::is_loaded)
            && self
                .measurements
                .iter()
                .any(ui::measurement::State::is_loaded)
    }

    fn compute_impulse_response(&mut self, id: ui::measurement::Id) -> Task<Message> {
        let loopback = self.loopback.as_ref().unwrap();

        let measurement = self
            .measurements
            .iter_mut()
            .filter_map(ui::measurement::State::loaded_mut)
            .find(|m| m.id == id)
            .unwrap();

        compute_impulse_response(loopback, measurement)
    }
}

fn impulse_response_item(
    selected: Option<ui::measurement::Id>,
    measurement: &ui::measurement::Loaded,
) -> Element<'_, Message> {
    let id = measurement.id;
    let is_active = selected.is_some_and(|selected| selected == id);

    let entry = {
        let dt: DateTime<Utc> = measurement.data.modified.into();
        let ir_btn = button(
            column![
                text(&measurement.name)
                    .size(14)
                    .wrapping(Wrapping::WordOrGlyph),
                text!("{}", dt.format("%x %X")).size(10)
            ]
            .clip(true)
            .spacing(6),
        )
        .on_press_with(move || Message::ImpulseResponseSelected(id))
        .width(Length::Fill)
        .style(move |theme: &Theme, status| {
            let background = theme.extended_palette().background;
            let base = button::subtle(theme, status);

            if is_active {
                base.with_background(background.weak.color)
            } else {
                base
            }
        });

        let save_btn = button(icon::download().size(10))
            .style(button::secondary)
            .on_press_with(move || Message::SaveImpulseResponseFileDialog(id));

        let content = row![
            ir_btn,
            rule::vertical(1.0),
            right(save_btn).width(Length::Shrink).padding([0, 6])
        ];

        sidebar::item(content, is_active)
    };

    match measurement.analysis.impulse_response.progress() {
        ui::impulse_response::Progress::None => entry,
        ui::impulse_response::Progress::Computing => {
            impulse_response::processing_overlay("Impulse Response", entry)
        }
        ui::impulse_response::Progress::Finished => entry,
    }
}

fn compute_impulse_response(
    loopback: &ui::Loopback,
    measurement: &mut ui::measurement::Loaded,
) -> Task<Message> {
    let Some(loopback) = loopback.loaded() else {
        return Task::none();
    };

    if measurement.analysis.impulse_response.result().is_some() {
        return Task::none();
    }

    measurement.analysis.impulse_response = ui::impulse_response::State::Computing;

    Task::perform(
        data::impulse_response::compute(loopback.clone(), measurement.data.clone()),
        Message::ImpulseResponseComputed.with(measurement.id),
    )
}

impl Default for Main {
    fn default() -> Self {
        Self {
            state: State::CollectingMeasuremnts { recording: None },
            loopback: None,
            measurements: vec![],
            project_path: None,
            selected: None,
            smoothing: frequency_response::Smoothing::default(),
            modal: Modal::None,
            signal_cache: canvas::Cache::default(),
            zoom: chart::Zoom::default(),
            offset: chart::Offset::default(),
        }
    }
}

async fn choose_impulse_response_file_path() -> Option<Arc<Path>> {
    rfd::AsyncFileDialog::new()
        .set_title("Save Impulse Response ...")
        .add_filter("wav", &["wav", "wave"])
        .add_filter("all", &["*"])
        .save_file()
        .await
        .as_ref()
        .map(|h| h.path().to_path_buf().into())
}

async fn save_impulse_response(path: Arc<Path>, impulse_response: ui::ImpulseResponse) {
    tokio::task::spawn_blocking(move || {
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: impulse_response.sample_rate.into(),
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        };

        let mut writer = hound::WavWriter::create(path, spec).unwrap();
        for s in impulse_response.data.iter().copied() {
            writer.write_sample(s).unwrap();
        }
        writer.finalize().unwrap();
    })
    .await
    .unwrap();
}

fn compute_frequency_response(
    loopback: &ui::Loopback,
    measurement: &mut ui::measurement::Loaded,
    window: &Window<Samples>,
) -> Task<Message> {
    let id = measurement.id;

    if let Some(impulse_response) = measurement.analysis.impulse_response.result() {
        measurement.analysis.frequency_response.progress =
            ui::frequency_response::Progress::Computing;

        Task::perform(
            data::frequency_response::compute(impulse_response.origin.clone(), window.clone()),
            Message::FrequencyResponseComputed.with(id),
        )
    } else {
        compute_impulse_response(loopback, measurement)
    }
}

fn compute_spectral_decay(
    loopback: &ui::Loopback,
    measurement: &mut ui::measurement::Loaded,
) -> Task<Message> {
    if measurement.analysis.spectral_decay.result().is_some() {
        return Task::none();
    }

    if let Some(impulse_response) = measurement.analysis.impulse_response.result() {
        measurement.analysis.spectral_decay = ui::spectral_decay::State::Computing;

        Task::perform(
            data::spectral_decay::compute(
                impulse_response.origin.clone(),
                spectral_decay::Preferences::default(),
            ),
            Message::SpectralDecayComputed.with(measurement.id),
        )
    } else {
        compute_impulse_response(loopback, measurement)
    }
}

fn compute_spectrogram(
    loopback: &ui::Loopback,
    measurement: &mut ui::measurement::Loaded,
) -> Task<Message> {
    if measurement.analysis.spectrogram.result().is_some() {
        return Task::none();
    }

    if let Some(impulse_response) = measurement.analysis.impulse_response.result() {
        measurement.analysis.spectrogram = ui::spectrogram::State::Computing;

        Task::perform(
            data::spectrogram::compute(
                impulse_response.origin.clone(),
                spectrogram::Preferences::default(),
            ),
            Message::SpectrogramComputed.with(measurement.id),
        )
    } else {
        compute_impulse_response(loopback, measurement)
    }
}

impl TabId {
    pub fn iter() -> impl Iterator<Item = Self> {
        [
            TabId::Measurements,
            TabId::ImpulseResponses,
            TabId::FrequencyResponses,
            TabId::SpectralDecay,
            TabId::Spectrogram,
        ]
        .into_iter()
    }

    pub fn view<'a>(self, is_analysing: bool) -> Element<'a, Message> {
        let mut row = row![];

        for tab in TabId::iter() {
            let is_active = self == tab;

            let is_enabled = match tab {
                TabId::Measurements => true,
                TabId::ImpulseResponses
                | TabId::FrequencyResponses
                | TabId::SpectralDecay
                | TabId::Spectrogram => is_analysing,
            };

            let button = button(text(tab.to_string()).size(16))
                .padding(10)
                .style(move |theme: &Theme, status| {
                    if is_active {
                        let palette = theme.extended_palette();

                        button::Style {
                            background: Some(palette.background.base.color.into()),
                            text_color: palette.background.base.text,
                            ..button::text(theme, status)
                        }
                    } else {
                        button::text(theme, status)
                    }
                })
                .on_press_maybe(if is_enabled {
                    Some(Message::TabSelected(tab))
                } else {
                    None
                });

            row = row.push(button);
        }

        row.spacing(5).align_y(Alignment::Center).into()
    }
}

impl Display for TabId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let label = match self {
            TabId::Measurements => "Measurements",
            TabId::ImpulseResponses => "Impulse Responses",
            TabId::FrequencyResponses => "Frequency Responses",
            TabId::SpectralDecay => "Spectral Decay",
            TabId::Spectrogram => "Spectorgram",
        };

        write!(f, "{}", label)
    }
}

impl From<&Tab> for TabId {
    fn from(tab: &Tab) -> Self {
        match tab {
            Tab::Measurements { .. } => TabId::Measurements,
            Tab::ImpulseResponses { .. } => TabId::ImpulseResponses,
            Tab::FrequencyResponses => TabId::FrequencyResponses,
            Tab::SpectralDecay => TabId::SpectralDecay,
            Tab::Spectrogram => TabId::Spectrogram,
        }
    }
}

fn modal<'a, Message>(
    base: impl Into<Element<'a, Message>>,
    content: impl Into<Element<'a, Message>>,
) -> Element<'a, Message>
where
    Message: Clone + 'a,
{
    stack![
        base.into(),
        opaque(center(opaque(content)).style(|_theme| {
            container::Style {
                background: Some(
                    Color {
                        a: 0.8,
                        ..Color::BLACK
                    }
                    .into(),
                ),
                ..container::Style::default()
            }
        }))
    ]
    .into()
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

        column!(header, rule::horizontal(1))
            .extend(self.entries)
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

#[derive(Debug, Clone, thiserror::Error)]
pub enum PickAndSaveError {
    #[error("dialog closed")]
    DialogClosed,
    #[error(transparent)]
    File(#[from] project::Error),
}

async fn pick_project_file() -> Result<PathBuf, PickAndSaveError> {
    let handle = rfd::AsyncFileDialog::new()
        .set_title("Save project file ...")
        .save_file()
        .await
        .ok_or(PickAndSaveError::DialogClosed)?;

    Ok(handle.path().to_path_buf())
}
