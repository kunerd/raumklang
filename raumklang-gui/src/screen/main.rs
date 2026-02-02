mod chart;
mod frequency_response;
mod impulse_response;
mod modal;
mod recording;
mod tab;

use modal::Modal;
use tab::Tab;

use crate::{
    PickAndLoadError,
    data::{
        self, Project, RecentProjects, SampleRate, Samples, Window, project, spectrogram, window,
    },
    icon, load_project, log,
    screen::main::{
        chart::waveform,
        modal::{SpectralDecayConfig, pending_window, spectral_decay_config, spectrogram_config},
    },
    ui::{self, Analysis, Loopback, Measurement, measurement},
    widget::{processing_overlay, sidebar},
};
use raumklang_core::dbfs;

use impulse_response::ChartOperation;
use recording::Recording;

use chrono::{DateTime, Utc};
use generic_overlay::generic_overlay::{dropdown_menu, dropdown_root};

use iced::{
    Alignment::{self, Center},
    Color, Element, Function, Length, Subscription, Task, Theme,
    alignment::{Horizontal, Vertical},
    keyboard, padding,
    widget::{
        Button, button, canvas, center, column, container, opaque, pick_list, row, rule,
        scrollable, stack, text,
    },
};
use prism::{Axis, Chart, Labels, axis, line_series};
use rfd::FileHandle;

use std::{
    collections::BTreeMap,
    mem,
    path::{Path, PathBuf},
    sync::Arc,
};

pub struct Main {
    state: State,
    modal: Modal,
    recording: Option<Recording>,
    selected: Option<measurement::Selected>,

    loopback: Option<Loopback>,
    measurements: measurement::List,

    project_path: Option<PathBuf>,

    signal_cache: canvas::Cache,
    zoom: chart::Zoom,
    offset: chart::Offset,

    smoothing: frequency_response::Smoothing,
    window: Option<Window<Samples>>,

    ir_chart: impulse_response::Chart,
    spectrogram: Spectrogram,

    spectral_decay_config: data::spectral_decay::Config,
    spectrogram_config: spectrogram::Preferences,
}

#[allow(clippy::large_enum_variant)]
#[derive(Default)]
enum State {
    #[default]
    Collecting,
    Analysing {
        active_tab: Tab,
        selected: Option<measurement::Id>,
        analyses: BTreeMap<measurement::Id, Analysis>,
    },
}

impl State {
    pub fn analysis() -> Self {
        Self::Analysing {
            active_tab: Tab::default(),
            selected: None,
            analyses: BTreeMap::new(),
        }
    }

    fn active_tab(&self) -> Option<&Tab> {
        match &self {
            State::Collecting => None,
            State::Analysing { active_tab, .. } => Some(active_tab),
        }
    }
}

#[derive(Debug, Default)]
struct Spectrogram {
    pub zoom: chart::Zoom,
    pub offset: chart::Offset,
    pub cache: canvas::Cache,
}

#[derive(Debug, Clone)]
pub enum Message {
    NewProject,
    LoadProject,
    ProjectLoaded(Result<(Arc<data::Project>, PathBuf), PickAndLoadError>),
    SaveProject,
    ProjectSaved(Result<PathBuf, project::Error>),
    LoadRecentProject(usize),

    LoadLoopback,
    LoopbackLoaded(Loopback),
    LoadMeasurement,
    MeasurementLoaded(Measurement),
    Measurement(measurement::Message),

    OpenTab(tab::Id),
    ImpulseResponseComputed(measurement::Id, data::ImpulseResponse),
    SaveImpulseResponseToFile(measurement::Id, Option<Arc<Path>>),

    FrequencyResponseComputed(measurement::Id, data::FrequencyResponse),
    ImpulseResponseSaved(measurement::Id, Arc<Path>),
    ImpulseResponseChart(impulse_response::ChartOperation),
    ImpulseResponse(ui::measurement::Id, ui::impulse_response::Message),

    FrequencyResponseToggled(measurement::Id, bool),
    ChangeSmoothing(frequency_response::Smoothing),
    FrequencyResponseSmoothed(measurement::Id, Box<[f32]>),

    ShiftKeyPressed,
    ShiftKeyReleased,

    MeasurementChart(waveform::Interaction),

    Recording(recording::Message),
    StartRecording(recording::Kind),

    OpenSpectralDecayConfig,
    SpectralDecayConfig(spectral_decay_config::Message),
    SpectralDecayComputed(measurement::Id, data::SpectralDecay),

    OpenSpectrogramConfig,
    SpectrogramConfig(spectrogram_config::Message),
    SpectrogramComputed(measurement::Id, data::Spectrogram),
    Spectrogram(chart::spectrogram::Interaction),

    PendingWindow(pending_window::Message),
}

impl Main {
    pub fn from_project(path: PathBuf, project: data::Project) -> (Self, Task<Message>) {
        let load_loopback = project
            .loopback
            .map(|loopback| {
                Task::perform(
                    Loopback::from_file(loopback.0.path),
                    Message::LoopbackLoaded,
                )
            })
            .unwrap_or_default();

        let load_measurements = project.measurements.into_iter().map(|measurement| {
            Task::perform(
                Measurement::from_file(measurement.path),
                Message::MeasurementLoaded,
            )
        });

        (
            Self {
                project_path: Some(path),
                ..Default::default()
            },
            Task::batch([load_loopback, Task::batch(load_measurements)]),
        )
    }

    pub fn update(&mut self, recent_projects: &mut RecentProjects, msg: Message) -> Task<Message> {
        match msg {
            Message::NewProject => {
                *self = Self::default();
                Task::none()
            }
            Message::LoadProject => Task::future(pick_project_file_to_load())
                .and_then(|path| Task::perform(load_project(path), Message::ProjectLoaded)),
            Message::LoadRecentProject(index) => {
                let Some(path) = recent_projects.get(index) else {
                    return Task::none();
                };

                Task::perform(load_project(path.clone()), Message::ProjectLoaded)
            }
            Message::ProjectLoaded(Ok((project, path))) => {
                let Some(project) = Arc::into_inner(project) else {
                    return Task::none();
                };

                let (view, tasks) = Self::from_project(path, project);

                *self = view;
                tasks
            }
            Message::SaveProject => {
                let loopback = self
                    .loopback
                    .as_ref()
                    .cloned()
                    .and_then(|l| l.path)
                    .map(|path| project::Loopback(project::Measurement { path }));

                let measurements = self
                    .measurements
                    .loaded()
                    .flat_map(|m| m.path.as_ref())
                    .cloned()
                    .map(|path| project::Measurement { path })
                    .collect();

                let project = Project {
                    loopback,
                    measurements,
                };

                if let Some(path) = self.project_path.as_ref() {
                    let path = path.clone();
                    Task::perform(project.save(path.clone()), |res| {
                        Message::ProjectSaved(res.map(|_| path))
                    })
                } else {
                    Task::future(pick_project_file_to_save()).and_then(move |path| {
                        Task::perform(project.clone().save(path.clone()), |res| {
                            Message::ProjectSaved(res.map(|_| path))
                        })
                    })
                }
            }
            Message::ProjectSaved(Ok(path)) => {
                self.project_path = Some(path.clone());
                recent_projects.insert(path);

                Task::future(recent_projects.clone().save()).discard()
            }
            Message::OpenTab(tab) => {
                let State::Analysing { ref active_tab, .. } = self.state else {
                    return Task::none();
                };

                if let Tab::ImpulseResponses { pending_window } = active_tab
                    && !matches!(tab, tab::Id::ImpulseResponses)
                    && self
                        .window
                        .as_ref()
                        .is_none_or(|window| pending_window != window)
                {
                    self.modal = Modal::PendingWindow { goto_tab: tab };
                    return Task::none();
                }

                match tab {
                    tab::Id::Measurements => {
                        let State::Analysing {
                            active_tab: ref mut tab,
                            ..
                        } = self.state
                        else {
                            return Task::none();
                        };

                        *tab = Tab::Measurements;

                        Task::none()
                    }
                    tab::Id::ImpulseResponses => {
                        let State::Analysing {
                            active_tab: ref mut tab,
                            ..
                        } = self.state
                        else {
                            return Task::none();
                        };

                        let Some(window) = &self.window else {
                            return Task::none();
                        };

                        *tab = Tab::ImpulseResponses {
                            pending_window: window.clone(),
                        };

                        return Task::none();
                    }
                    tab::Id::FrequencyResponses => {
                        let State::Analysing {
                            ref mut active_tab,
                            ref mut analyses,
                            ..
                        } = self.state
                        else {
                            return Task::none();
                        };

                        *active_tab = Tab::FrequencyResponses {
                            cache: canvas::Cache::new(),
                        };

                        let tasks = self.measurements.loaded().map(Measurement::id).map(|id| {
                            compute_frequency_response(
                                analyses,
                                id,
                                self.loopback.as_ref(),
                                &self.measurements,
                                self.window.as_ref().cloned().unwrap(),
                            )
                        });

                        Task::batch(tasks)
                    }
                    tab::Id::SpectralDecays => {
                        let State::Analysing {
                            selected,
                            ref mut active_tab,
                            ref mut analyses,
                            ..
                        } = self.state
                        else {
                            return Task::none();
                        };

                        *active_tab = Tab::SpectralDecays {
                            cache: canvas::Cache::new(),
                        };

                        if let Some(id) = selected {
                            compute_spectral_decay(
                                id,
                                analyses,
                                self.spectral_decay_config,
                                self.loopback.as_ref(),
                                &self.measurements,
                            )
                        } else {
                            Task::none()
                        }
                    }
                    tab::Id::Spectrograms => {
                        let State::Analysing {
                            selected,
                            ref mut active_tab,
                            ref mut analyses,
                            ..
                        } = self.state
                        else {
                            return Task::none();
                        };

                        *active_tab = Tab::Spectrograms;
                        self.spectrogram.cache.clear();

                        if let Some(id) = selected {
                            compute_spectrogram(
                                id,
                                analyses,
                                &self.spectrogram_config,
                                self.loopback.as_ref(),
                                &self.measurements,
                            )
                        } else {
                            Task::none()
                        }
                    }
                }
            }
            Message::LoadLoopback => Task::future(pick_measurement_file("Load Loopback ..."))
                .and_then(|path| Task::perform(Loopback::from_file(path), Message::LoopbackLoaded)),
            Message::LoadMeasurement => Task::future(pick_measurement_file("Load measurement ..."))
                .and_then(|path| {
                    Task::perform(Measurement::from_file(path), Message::MeasurementLoaded)
                }),
            Message::LoopbackLoaded(loopback) => {
                self.window = loopback
                    .loaded()
                    .map(raumklang_core::Loopback::sample_rate)
                    .map(SampleRate::from)
                    .map(Window::new)
                    .map(Into::into);

                self.loopback = Some(loopback);

                if !self.measurements.is_empty() {
                    self.state = State::analysis();
                }

                Task::none()
            }
            Message::MeasurementLoaded(measurement) => {
                let is_loopback_loaded = self.loopback.as_ref().is_some_and(Loopback::is_loaded);

                if is_loopback_loaded && self.measurements.is_empty() {
                    self.state = State::analysis();
                }

                self.measurements.push(measurement);

                Task::none()
            }
            Message::Measurement(msg) => {
                match msg {
                    measurement::Message::Select(selected) => {
                        self.selected = Some(selected);
                        self.signal_cache.clear();
                    }
                    measurement::Message::Remove(id) => {
                        self.measurements.remove(id);

                        if self.measurements.loaded().next().is_none() {
                            self.state = State::Collecting
                        }

                        if let State::Analysing {
                            ref mut analyses, ..
                        } = self.state
                        {
                            analyses.remove(&id);
                        }
                    }
                };

                Task::none()
            }
            Message::MeasurementChart(interaction) => {
                match interaction {
                    waveform::Interaction::ZoomChanged(zoom) => self.zoom = zoom,
                    waveform::Interaction::OffsetChanged(offset) => self.offset = offset,
                }

                self.signal_cache.clear();

                Task::none()
            }
            Message::ImpulseResponse(id, ui::impulse_response::Message::Select) => {
                let State::Analysing {
                    active_tab: tab,
                    selected,
                    analyses,
                    ..
                } = &mut self.state
                else {
                    return Task::none();
                };

                *selected = Some(id);
                self.ir_chart.data_cache.clear();

                match tab {
                    Tab::Measurements { .. } => Task::none(),
                    Tab::ImpulseResponses { .. } => compute_impulse_response(
                        analyses,
                        id,
                        self.loopback.as_ref(),
                        &self.measurements,
                    ),
                    Tab::FrequencyResponses { .. } => Task::none(),
                    Tab::SpectralDecays { .. } => compute_spectral_decay(
                        id,
                        analyses,
                        self.spectral_decay_config,
                        self.loopback.as_ref(),
                        &self.measurements,
                    ),

                    Tab::Spectrograms => compute_spectrogram(
                        id,
                        analyses,
                        &self.spectrogram_config,
                        self.loopback.as_ref(),
                        &self.measurements,
                    ),
                }
            }
            Message::ImpulseResponse(id, ui::impulse_response::Message::Save) => {
                let State::Analysing { .. } = self.state else {
                    return Task::none();
                };

                Task::perform(
                    choose_impulse_response_file_path(),
                    Message::SaveImpulseResponseToFile.with(id),
                )
            }
            Message::SaveImpulseResponseToFile(id, path) => {
                let Some(path) = path else {
                    return Task::none();
                };

                let State::Analysing {
                    ref mut analyses, ..
                } = self.state
                else {
                    return Task::none();
                };

                let analysis = analyses.entry(id).or_default();
                if let Some(ir) = analysis.impulse_response.result().cloned() {
                    Task::perform(save_impulse_response(path.clone(), ir.clone()), move |_| {
                        Message::ImpulseResponseSaved(id, path)
                    })
                } else {
                    compute_impulse_response(
                        analyses,
                        id,
                        self.loopback.as_ref(),
                        &self.measurements,
                    )
                    .chain(Task::done(Message::SaveImpulseResponseToFile(
                        id,
                        Some(path),
                    )))
                }
            }
            Message::ImpulseResponseSaved(id, path) => {
                eprintln!("IR (#{:?}) saved to: {:?}", id, path);

                Task::none()
            }
            Message::ImpulseResponseComputed(id, impulse_response) => {
                let State::Analysing {
                    ref active_tab,
                    ref mut analyses,
                    ..
                } = self.state
                else {
                    return Task::none();
                };

                analyses.entry(id).and_modify(|analysis| {
                    analysis.impulse_response =
                        ui::impulse_response::State::from_data(impulse_response);
                });

                match active_tab {
                    Tab::Measurements { .. } => Task::none(),
                    Tab::ImpulseResponses { .. } => Task::none(),
                    Tab::FrequencyResponses { .. } => compute_frequency_response(
                        analyses,
                        id,
                        self.loopback.as_ref(),
                        &self.measurements,
                        self.window.as_ref().cloned().unwrap(),
                    ),
                    Tab::SpectralDecays { .. } => compute_spectral_decay(
                        id,
                        analyses,
                        self.spectral_decay_config,
                        self.loopback.as_ref(),
                        &self.measurements,
                    ),
                    Tab::Spectrograms => compute_spectrogram(
                        id,
                        analyses,
                        &self.spectrogram_config,
                        self.loopback.as_ref(),
                        &self.measurements,
                    ),
                }
            }
            Message::PendingWindow(action) => {
                let State::Analysing {
                    active_tab,
                    analyses,
                    ..
                } = &mut self.state
                else {
                    return Task::none();
                };

                let Modal::PendingWindow { goto_tab } = mem::take(&mut self.modal) else {
                    return Task::none();
                };

                let tab = mem::take(active_tab);
                if let Tab::ImpulseResponses { pending_window } = tab {
                    match action {
                        pending_window::Message::Discard => self.ir_chart.overlay_cache.clear(),
                        pending_window::Message::Apply => {
                            self.window = Some(pending_window);
                            analyses.values_mut().for_each(|a| *a = Analysis::default());
                        }
                    }
                }

                self.update(recent_projects, Message::OpenTab(goto_tab))
            }
            Message::FrequencyResponseComputed(id, new_fr) => {
                log::debug!("Frequency response computed: {id}");

                let State::Analysing {
                    ref mut analyses,
                    active_tab: Tab::FrequencyResponses { ref cache },
                    ..
                } = self.state
                else {
                    return Task::none();
                };

                let task = if let Some(fraction) = self.smoothing.fraction() {
                    Task::perform(
                        frequency_response::smooth_frequency_response(new_fr.clone(), fraction),
                        Message::FrequencyResponseSmoothed.with(id),
                    )
                } else {
                    Task::none()
                };

                let analysis = analyses.entry(id).or_default();
                analysis.frequency_response.set_result(new_fr);
                cache.clear();

                task
            }
            Message::FrequencyResponseToggled(id, state) => {
                let State::Analysing {
                    ref mut analyses,
                    active_tab: Tab::FrequencyResponses { ref cache },
                    ..
                } = self.state
                else {
                    return Task::none();
                };

                let Some(fr) = analyses.get_mut(&id).map(Analysis::frequency_response_mut) else {
                    return Task::none();
                };

                fr.is_shown = state;
                cache.clear();

                Task::none()
            }
            Message::ChangeSmoothing(smoothing) => {
                let State::Analysing {
                    ref mut analyses,
                    active_tab: Tab::FrequencyResponses { ref cache },
                    ..
                } = self.state
                else {
                    return Task::none();
                };

                self.smoothing = smoothing;

                if let Some(fraction) = smoothing.fraction() {
                    let tasks = analyses.iter().flat_map(|(id, analysis)| {
                        let fr = analysis.frequency_response.result()?;

                        Some(Task::perform(
                            frequency_response::smooth_frequency_response(fr.clone(), fraction),
                            Message::FrequencyResponseSmoothed.with(*id),
                        ))
                    });

                    Task::batch(tasks)
                } else {
                    analyses
                        .values_mut()
                        .map(Analysis::frequency_response_mut)
                        .for_each(|fr| fr.smoothed = None);

                    cache.clear();

                    Task::none()
                }
            }
            Message::FrequencyResponseSmoothed(id, smoothed) => {
                let State::Analysing {
                    ref mut analyses,
                    active_tab: Tab::FrequencyResponses { ref cache },
                    ..
                } = self.state
                else {
                    return Task::none();
                };

                if let Some(fr) = analyses.get_mut(&id).map(|a| &mut a.frequency_response) {
                    fr.smoothed = Some(smoothed);
                    cache.clear();
                }

                Task::none()
            }

            Message::SpectralDecayComputed(id, sd) => {
                let State::Analysing {
                    ref mut analyses,
                    active_tab: Tab::SpectralDecays { ref cache },
                    ..
                } = self.state
                else {
                    return Task::none();
                };

                let Some(analysis) = analyses.get_mut(&id) else {
                    return Task::none();
                };

                analysis.spectral_decay.set_result(sd);
                cache.clear();

                Task::none()
            }
            Message::OpenSpectralDecayConfig => {
                self.modal = Modal::SpectralDecayConfig(SpectralDecayConfig::new(
                    self.spectral_decay_config,
                ));

                Task::none()
            }
            Message::SpectralDecayConfig(msg) => {
                let Modal::SpectralDecayConfig(modal) = &mut self.modal else {
                    return Task::none();
                };

                let State::Analysing {
                    selected,
                    ref mut analyses,
                    ..
                } = self.state
                else {
                    return Task::none();
                };

                match modal.update(msg) {
                    spectral_decay_config::Action::None => Task::none(),
                    spectral_decay_config::Action::Discard => {
                        self.modal = Modal::None;
                        Task::none()
                    }
                    spectral_decay_config::Action::Apply(config) => {
                        self.modal = Modal::None;
                        self.spectral_decay_config = config;

                        analyses.values_mut().for_each(|a| a.spectral_decay.reset());

                        selected
                            .map(|id| {
                                compute_spectral_decay(
                                    id,
                                    analyses,
                                    config,
                                    self.loopback.as_ref(),
                                    &self.measurements,
                                )
                            })
                            .unwrap_or_default()
                    }
                }
            }
            Message::SpectrogramComputed(id, spectrogram) => {
                let State::Analysing {
                    selected,
                    ref mut analyses,
                    ..
                } = self.state
                else {
                    return Task::none();
                };

                let analysis = analyses.entry(id).or_default();
                analysis.spectrogram.set_result(spectrogram);

                if selected.is_some_and(|selected| selected == id) {
                    self.spectrogram.cache.clear();
                }

                Task::none()
            }
            Message::Spectrogram(interaction) => {
                match interaction {
                    chart::spectrogram::Interaction::ZoomChanged(zoom) => {
                        self.spectrogram.zoom = zoom
                    }
                    chart::spectrogram::Interaction::OffsetChanged(offset) => {
                        self.spectrogram.offset = offset
                    }
                }

                self.spectrogram.cache.clear();

                Task::none()
            }
            Message::OpenSpectrogramConfig => {
                self.modal = Modal::SpectrogramConfig(modal::SpectrogramConfig::new(
                    self.spectrogram_config,
                ));

                Task::none()
            }
            Message::SpectrogramConfig(message) => {
                let Modal::SpectrogramConfig(config) = &mut self.modal else {
                    return Task::none();
                };

                let State::Analysing {
                    selected, analyses, ..
                } = &mut self.state
                else {
                    return Task::none();
                };

                match config.update(message) {
                    spectrogram_config::Action::None => Task::none(),
                    spectrogram_config::Action::Close => {
                        self.modal = Modal::None;

                        Task::none()
                    }
                    spectrogram_config::Action::ConfigChanged(preferences) => {
                        self.modal = Modal::None;
                        self.spectrogram_config = preferences;

                        analyses.values_mut().for_each(|a| a.spectrogram.reset());

                        if let Some(id) = selected {
                            analyses
                                .get_mut(&id)
                                .and_then(move |a| {
                                    a.spectrogram.compute(&a.impulse_response, &preferences)
                                })
                                .map(|f| Task::perform(f, Message::SpectrogramComputed.with(*id)))
                                .unwrap_or_default()
                        } else {
                            Task::none()
                        }
                    }
                }
            }
            Message::ImpulseResponseChart(operation) => {
                let State::Analysing {
                    active_tab: Tab::ImpulseResponses { pending_window },
                    ..
                } = &mut self.state
                else {
                    return Task::none();
                };

                if let ChartOperation::Interaction(ref interaction) = operation {
                    match interaction {
                        chart::Interaction::HandleMoved(index, new_pos) => {
                            let mut handles = window::Handles::from(&*pending_window);
                            handles.update(*index, *new_pos);
                            pending_window.update(handles);
                        }
                        chart::Interaction::ZoomChanged(zoom) => {
                            self.ir_chart.zoom = *zoom;
                        }
                        chart::Interaction::OffsetChanged(offset) => {
                            self.ir_chart.offset = *offset;
                        }
                    }
                }
                self.ir_chart.update(operation);

                Task::none()
            }
            Message::StartRecording(kind) => {
                self.recording = Some(Recording::new(kind));
                Task::none()
            }
            Message::Recording(msg) => {
                let Some(recording) = &mut self.recording else {
                    return Task::none();
                };

                match recording.update(msg) {
                    recording::Action::None => Task::none(),
                    recording::Action::Cancel => {
                        self.recording = None;
                        Task::none()
                    }
                    recording::Action::Task(task) => task.map(Message::Recording),
                    recording::Action::Finished(result) => {
                        match result {
                            recording::Result::Loopback(loopback) => {
                                self.loopback =
                                    Some(ui::Loopback::new("Loopback".to_string(), loopback));
                            }
                            recording::Result::Measurement(measurement) => {
                                self.measurements.push(ui::Measurement::new(
                                    "Measurement".to_string(),
                                    None,
                                    Some(measurement),
                                ));
                            }
                        }

                        self.recording = None;
                        Task::none()
                    }
                }
            }
            Message::ShiftKeyPressed => {
                self.ir_chart.shift_key_pressed();
                Task::none()
            }
            Message::ShiftKeyReleased => {
                self.ir_chart.shift_key_released();
                Task::none()
            }
            _ => Task::none(),
        }
    }

    // pub fn update(
    //
    //     &mut self,
    //     recent_projects: &mut RecentProjects,
    //     message: Message,
    // ) -> Task<Message> {
    //     match message {
    //         Message::TabSelected(id) => {
    //             let State::Analysing {
    //                 ref mut active_tab,
    //                 ref window,
    //                 ref mut selected,
    //                 ref loopback,
    //                 ..
    //             } = self.state
    //             else {
    //                 return Task::none();
    //             };

    //             let (tab, tasks) = match (&active_tab, id) {
    //                 (Tab::Measurements { .. }, TabId::Measurements)
    //                 | (Tab::ImpulseResponses { .. }, TabId::ImpulseResponses)
    //                 | (Tab::FrequencyResponses, TabId::FrequencyResponses) => return Task::none(),
    //                 (
    //                     Tab::ImpulseResponses {
    //                         ref window_settings,
    //                         ..
    //                     },
    //                     tab_id,
    //                 ) if window_settings.window != *window => {
    //                     self.modal = Modal::PendingWindow { goto_tab: tab_id };
    //                     return Task::none();
    //                 }
    //                 (_, TabId::Measurements) => {
    //                     (Tab::Measurements { recording: None }, Task::none())
    //                 }
    //                 (_, TabId::ImpulseResponses) => (
    //                     Tab::ImpulseResponses {
    //                         window_settings: WindowSettings::new(window.clone()),
    //                     },
    //                     Task::none(),
    //                 ),
    //                 (_, TabId::FrequencyResponses) => {
    //                     let tasks = self.measurements.loaded_mut().map(|measurement| {
    //                         compute_frequency_response(loopback, measurement, window)
    //                     });
    //                     (Tab::FrequencyResponses, Task::batch(tasks))
    //                 }
    //                 (_, TabId::SpectralDecay) => (Tab::SpectralDecay, Task::none()),
    //                 (_, TabId::Spectrogram) => (Tab::Spectrogram, Task::none()),
    //             };

    //             *active_tab = tab;
    //             *selected = None;

    //             tasks
    //         }
    //         Message::Measurements(message) => {
    //             match message {
    //                 measurement::Message::Load(kind) => {
    //                     let dialog_caption = kind.to_string();

    //                     return Task::perform(
    //                         measurement::pick_file_and_load_signal(dialog_caption, kind),
    //                         measurement::Message::Loaded,
    //                     )
    //                     .map(Message::Measurements);
    //                 }
    //                 measurement::Message::Loaded(Ok(result)) => match Arc::into_inner(result) {
    //                     Some(measurement::LoadedKind::Loopback(new)) => match &mut self.state {
    //                         State::CollectingMeasuremnts { loopback, .. } => *loopback = Some(new),
    //                         State::Analysing { loopback, .. } => *loopback = new,
    //                     },
    //                     Some(measurement::LoadedKind::Normal(measurement)) => {
    //                         self.measurements.push(measurement)
    //                     }
    //                     None => {}
    //                 },
    //                 measurement::Message::Loaded(Err(err)) => {
    //                     log::error!("{err}");
    //                 }
    //                 measurement::Message::Remove(id) => {
    //                     self.measurements.remove(id);
    //                 }
    //                 measurement::Message::Select(selected) => {
    //                     self.selected = Some(selected);
    //                     self.signal_cache.clear();
    //                 }
    //             }

    //             let is_analysing_possible = self.analysing_possible();
    //             let state = std::mem::take(&mut self.state);
    //             self.state = match (state, is_analysing_possible) {
    //                 (
    //                     State::CollectingMeasuremnts {
    //                         recording,
    //                         loopback,
    //                     },
    //                     false,
    //                 ) => State::CollectingMeasuremnts {
    //                     recording,
    //                     loopback,
    //                 },
    //                 (
    //                     State::CollectingMeasuremnts {
    //                         recording,
    //                         loopback: Some(loopback),
    //                     },
    //                     true,
    //                 ) => State::Analysing {
    //                     active_tab: Tab::Measurements { recording },
    //                     window: Window::new(SampleRate::from(
    //                         loopback
    //                             .loaded()
    //                             .map_or(44_100, |l| l.as_ref().sample_rate()),
    //                     ))
    //                     .into(),
    //                     selected: None,
    //                     charts: Charts::default(),
    //                     loopback,
    //                 },
    //                 (old_state, true) => old_state,
    //                 (State::Analysing { loopback, .. }, false) => State::CollectingMeasuremnts {
    //                     recording: None,
    //                     loopback: Some(loopback),
    //                 },
    //             };

    //             Task::none()
    //         }
    //         Message::ImpulseResponseSelected(id) => {
    //             log::debug!("Impulse response selected: {id}");

    //             let State::Analysing {
    //                 selected: ref mut selected_analysis,
    //                 ref mut charts,
    //                 ref active_tab,
    //                 ref loopback,
    //                 ..
    //             } = self.state
    //             else {
    //                 return Task::none();
    //             };

    //             *selected_analysis = Some(id);
    //             charts.impulse_responses.data_cache.clear();

    //             let Some(measurement) = self.measurements.get_mut(id) else {
    //                 return Task::none();
    //             };

    //             match active_tab {
    //                 Tab::Measurements { .. } => Task::none(),
    //                 Tab::ImpulseResponses { .. } => compute_impulse_response(loopback, measurement),
    //                 Tab::FrequencyResponses => Task::none(),
    //                 Tab::SpectralDecay => {
    //                     compute_spectral_decay(loopback, measurement, self.spectral_decay_config)
    //                 }
    //                 Tab::Spectrogram => {
    //                     compute_spectrogram(loopback, measurement, &self.spectrogram_config)
    //                 }
    //             }
    //         }
    //         Message::ImpulseResponseComputed(id, impulse_response) => {
    //             let State::Analysing {
    //                 window,
    //                 active_tab,
    //                 selected: selected_analysis,
    //                 charts,
    //                 loopback,
    //                 ..
    //             } = &mut self.state
    //             else {
    //                 return Task::none();
    //             };

    //             let impulse_response = ui::ImpulseResponse::from_data(impulse_response);

    //             let Some(measurement) = self.measurements.get_mut(id) else {
    //                 return Task::none();
    //             };

    //             let Some(analysis) = measurement.analysis_mut() else {
    //                 return Task::none();
    //             };

    //             analysis.impulse_response.computed(impulse_response.clone());

    //             if selected_analysis.is_some_and(|selected| selected == id) {
    //                 charts
    //                     .impulse_responses
    //                     .x_range
    //                     .get_or_insert(0.0..=impulse_response.data.len() as f32);

    //                 charts.impulse_responses.data_cache.clear();
    //             }

    //             if let Tab::FrequencyResponses = active_tab {
    //                 compute_frequency_response(loopback, measurement, window)
    //             } else if let Tab::SpectralDecay = active_tab {
    //                 compute_spectral_decay(loopback, measurement, self.spectral_decay_config)
    //             } else if let Tab::Spectrogram = active_tab {
    //                 compute_spectrogram(loopback, measurement, &self.spectrogram_config)
    //             } else {
    //                 Task::none()
    //             }
    //         }
    //         Message::ImpulseResponses(impulse_response::Message::Chart(operation)) => {
    //             let State::Analysing {
    //                 active_tab:
    //                     Tab::ImpulseResponses {
    //                         ref mut window_settings,
    //                         ..
    //                     },
    //                 ref mut charts,
    //                 ..
    //             } = self.state
    //             else {
    //                 return Task::none();
    //             };

    //             if let ChartOperation::Interaction(ref interaction) = operation {
    //                 match interaction {
    //                     chart::Interaction::HandleMoved(index, new_pos) => {
    //                         let mut handles: window::Handles = Into::into(&window_settings.window);
    //                         handles.update(*index, *new_pos);
    //                         window_settings.window.update(handles);
    //                     }
    //                     chart::Interaction::ZoomChanged(zoom) => {
    //                         charts.impulse_responses.zoom = *zoom;
    //                     }
    //                     chart::Interaction::OffsetChanged(offset) => {
    //                         charts.impulse_responses.offset = *offset;
    //                     }
    //                 }
    //             }

    //             charts.impulse_responses.update(operation);

    //             Task::none()
    //         }
    //         Message::FrequencyResponseComputed(id, frequency_response) => {
    //             log::debug!("Frequency response computed: {id}");

    //             let State::Analysing { ref mut charts, .. } = self.state else {
    //                 return Task::none();
    //             };

    //             let Some(measurement) = self.measurements.get_mut(id) else {
    //                 return Task::none();
    //             };

    //             let Some(analysis) = measurement.analysis_mut() else {
    //                 return Task::none();
    //             };

    //             analysis
    //                 .frequency_response
    //                 .computed(frequency_response.clone());

    //             charts.frequency_responses.cache.clear();

    //             if let Some(fraction) = self.smoothing.fraction() {
    //                 Task::perform(
    //                     frequency_response::smooth_frequency_response(
    //                         id,
    //                         frequency_response,
    //                         fraction,
    //                     ),
    //                     Message::FrequencyResponseSmoothed,
    //                 )
    //             } else {
    //                 Task::none()
    //             }
    //         }
    //         Message::FrequencyResponseToggled(id, state) => {
    //             let State::Analysing { ref mut charts, .. } = self.state else {
    //                 return Task::none();
    //             };

    //             let Some(frequency_response) = self
    //                 .measurements
    //                 .get_mut(id)
    //                 .and_then(ui::Measurement::analysis_mut)
    //                 .map(|analysis| &mut analysis.frequency_response)
    //             else {
    //                 return Task::none();
    //             };

    //             frequency_response.is_shown = state;

    //             charts.frequency_responses.cache.clear();

    //             Task::none()
    //         }
    //         Message::SmoothingChanged(smoothing) => {
    //             let State::Analysing { ref mut charts, .. } = self.state else {
    //                 return Task::none();
    //             };

    //             self.smoothing = smoothing;

    //             if let Some(fraction) = smoothing.fraction() {
    //                 let tasks = self.measurements.iter().flat_map(|measurement| {
    //                     let fr = measurement
    //                         .analysis()
    //                         .as_ref()
    //                         .and_then(|a| a.frequency_response.data.clone())?;

    //                     Some(Task::perform(
    //                         frequency_response::smooth_frequency_response(
    //                             measurement.id(),
    //                             fr,
    //                             fraction,
    //                         ),
    //                         Message::FrequencyResponseSmoothed,
    //                     ))
    //                 });

    //                 Task::batch(tasks)
    //             } else {
    //                 self.measurements
    //                     .iter_mut()
    //                     .flat_map(ui::Measurement::analysis_mut)
    //                     .map(|analysis| &mut analysis.frequency_response)
    //                     .for_each(|fr| fr.smoothed = None);

    //                 charts.frequency_responses.cache.clear();

    //                 Task::none()
    //             }
    //         }
    //         Message::FrequencyResponseSmoothed((id, smoothed_data)) => {
    //             let State::Analysing { ref mut charts, .. } = self.state else {
    //                 return Task::none();
    //             };

    //             let Some(frequency_response) = self
    //                 .measurements
    //                 .get_mut(id)
    //                 .and_then(ui::Measurement::analysis_mut)
    //                 .map(|analysis| &mut analysis.frequency_response)
    //             else {
    //                 return Task::none();
    //             };

    //             frequency_response.smoothed = Some(smoothed_data);
    //             charts.frequency_responses.cache.clear();

    //             Task::none()
    //         }
    //         Message::ShiftKeyPressed => {
    //             let State::Analysing { ref mut charts, .. } = self.state else {
    //                 return Task::none();
    //             };

    //             charts.impulse_responses.shift_key_pressed();

    //             Task::none()
    //         }
    //         Message::ShiftKeyReleased => {
    //             let State::Analysing { ref mut charts, .. } = self.state else {
    //                 return Task::none();
    //             };

    //             charts.impulse_responses.shift_key_released();

    //             Task::none()
    //         }
    //         Message::Modal(action) => {
    //             let Modal::PendingWindow { goto_tab } = std::mem::take(&mut self.modal) else {
    //                 return Task::none();
    //             };

    //             let State::Analysing {
    //                 ref mut active_tab,
    //                 ref mut window,
    //                 ..
    //             } = self.state
    //             else {
    //                 return Task::none();
    //             };

    //             let Tab::ImpulseResponses {
    //                 ref mut window_settings,
    //                 ..
    //             } = active_tab
    //             else {
    //                 return Task::none();
    //             };

    //             match action {
    //                 ModalAction::Discard => {
    //                     *window_settings = WindowSettings::new(window.clone());
    //                 }
    //                 ModalAction::Apply => {
    //                     self.measurements
    //                         .iter_mut()
    //                         .flat_map(ui::Measurement::analysis_mut)
    //                         .for_each(|analysis| {
    //                             analysis.frequency_response = ui::FrequencyResponse::default();
    //                             analysis.spectral_decay = ui::spectral_decay::State::default();
    //                             analysis.spectrogram = ui::spectrogram::State::default();
    //                         });

    //                     *window = window_settings.window.clone();
    //                 }
    //             }

    //             Task::done(Message::TabSelected(goto_tab))
    //         }
    //         Message::Recording(message) => {
    //             let (State::CollectingMeasuremnts {
    //                 ref mut recording,
    //                 loopback: Some(ref mut loopback),
    //             }
    //             | State::Analysing {
    //                 active_tab: Tab::Measurements { ref mut recording },
    //                 ref mut loopback,
    //                 ..
    //             }) = self.state
    //             else {
    //                 return Task::none();
    //             };

    //             if let Some(view) = recording {
    //                 match view.update(message) {
    //                     recording::Action::None => Task::none(),
    //                     recording::Action::Cancel => {
    //                         *recording = None;
    //                         Task::none()
    //                     }
    //                     recording::Action::Finished(result) => {
    //                         match result {
    //                             recording::Result::Loopback(signal) => {
    //                                 *loopback = ui::Loopback::new("Loopback".to_string(), signal)
    //                             }
    //                             recording::Result::Measurement(signal) => {
    //                                 self.measurements.push(ui::Measurement::new(
    //                                     "Measurement".to_string(),
    //                                     None,
    //                                     Some(signal),
    //                                 ))
    //                             }
    //                         }
    //                         *recording = None;
    //                         Task::none()
    //                     }
    //                     recording::Action::Task(task) => task.map(Message::Recording),
    //                 }
    //             } else {
    //                 Task::none()
    //             }
    //         }
    //         Message::StartRecording(kind) => match &mut self.state {
    //             State::CollectingMeasuremnts { recording, .. }
    //             | State::Analysing {
    //                 active_tab: Tab::Measurements { recording },
    //                 ..
    //             } => {
    //                 *recording = Some(Recording::new(kind));
    //                 Task::none()
    //             }
    //             _ => Task::none(),
    //         },
    //         Message::MeasurementChart(interaction) => {
    //             match interaction {
    //                 waveform::Interaction::ZoomChanged(zoom) => self.zoom = zoom,
    //                 waveform::Interaction::OffsetChanged(offset) => self.offset = offset,
    //             }

    //             self.signal_cache.clear();

    //             Task::none()
    //         }
    //         Message::SaveImpulseResponseFileDialog(id) => {
    //             Task::future(choose_impulse_response_file_path())
    //                 .and_then(Task::done)
    //                 .map(Message::SaveImpulseResponse.with(id))
    //         }
    //         Message::SaveImpulseResponse(id, path) => {
    //             let State::Analysing {
    //                 active_tab: Tab::ImpulseResponses { .. },
    //                 ..
    //             } = &self.state
    //             else {
    //                 return Task::none();
    //             };

    //             if let Some(impulse_response) = self
    //                 .measurements
    //                 .get(id)
    //                 .and_then(ui::Measurement::analysis)
    //                 .and_then(|analysis| analysis.impulse_response.result())
    //                 .cloned()
    //             {
    //                 Task::perform(
    //                     save_impulse_response(path.clone(), impulse_response),
    //                     |_| Message::ImpulseResponsesSaved(path),
    //                 )
    //             } else {
    //                 self.compute_impulse_response(id)
    //                     .chain(Task::done(Message::SaveImpulseResponse(id, path)))
    //             }
    //         }
    //         Message::ImpulseResponsesSaved(path) => {
    //             log::debug!("Impulse response saved to: {}", path.display());

    //             Task::none()
    //         }
    //         Message::NewProject => {
    //             *self = Self::default();

    //             Task::none()
    //         }
    //         Message::LoadProject => Task::perform(
    //             crate::pick_project_file().then(async |res| {
    //                 let path = res?;
    //                 load_project(path).await
    //             }),
    //             Message::ProjectLoaded,
    //         ),
    //         Message::ProjectLoaded(Ok((project, path))) => match Arc::into_inner(project) {
    //             Some(project) => {
    //                 recent_projects.insert(path.clone());

    //                 let (screen, tasks) = Self::from_project(path, project);
    //                 *self = screen;

    //                 Task::batch([
    //                     tasks,
    //                     Task::future(recent_projects.clone().save()).discard(),
    //                 ])
    //             }
    //             None => Task::none(),
    //         },
    //         Message::ProjectLoaded(Err(err)) => {
    //             log::debug!("Loading project failed: {err}");

    //             Task::none()
    //         }
    //         Message::RecentProject(id) => match recent_projects.get(id) {
    //             Some(path) => Task::perform(load_project(path.clone()), Message::ProjectLoaded),
    //             None => Task::none(),
    //         },
    //         Message::SaveProject => {
    //             let loopback = if let State::CollectingMeasuremnts { ref loopback, .. } = self.state
    //             {
    //                 loopback.as_ref()
    //             } else if let State::Analysing { ref loopback, .. } = self.state {
    //                 Some(loopback)
    //             } else {
    //                 None
    //             };

    //             let loopback = loopback
    //                 .as_ref()
    //                 .and_then(|l| l.path.clone())
    //                 .map(|path| project::Loopback(project::Measurement { path }));

    //             let measurements = self
    //                 .measurements
    //                 .iter()
    //                 .flat_map(|m| m.path.clone())
    //                 .map(|path| project::Measurement { path })
    //                 .collect();

    //             let project = Project {
    //                 loopback,
    //                 measurements,
    //             };

    //             if let Some(path) = self.project_path.clone() {
    //                 Task::perform(
    //                     project
    //                         .save(path.clone())
    //                         .map_ok(move |_| path)
    //                         .map_err(PickAndSaveError::File),
    //                     Message::ProjectSaved,
    //                 )
    //             } else {
    //                 Task::perform(
    //                     pick_project_file().then(async |res| {
    //                         let path = res?;
    //                         project.save(path.clone()).await?;

    //                         Ok(path)
    //                     }),
    //                     Message::ProjectSaved,
    //                 )
    //             }
    //         }
    //         Message::ProjectSaved(Ok(path)) => {
    //             log::debug!("Project saved.");

    //             self.project_path = Some(path);

    //             Task::none()
    //         }
    //         Message::ProjectSaved(Err(err)) => {
    //             log::debug!("Saving project failed: {err}");
    //             Task::none()
    //         }
    //         Message::SpectralDecayComputed(id, decay) => {
    //             log::debug!(
    //                 "Spectral decay for measurement (ID: {}) with: {} slices, computed.",
    //                 id,
    //                 decay.len()
    //             );

    //             let Some(spectral_decay) = self
    //                 .measurements
    //                 .get_mut(id)
    //                 .and_then(ui::Measurement::analysis_mut)
    //                 .map(|analysis| &mut analysis.spectral_decay)
    //             else {
    //                 return Task::none();
    //             };

    //             spectral_decay.computed(decay);

    //             if let State::Analysing { charts, .. } = &mut self.state {
    //                 charts.spectral_decay_cache.clear();
    //             };

    //             Task::none()
    //         }
    //         Message::Spectrogram(interaction) => {
    //             log::debug!("Spectrogram chart: {interaction:?}.");

    //             let State::Analysing { charts, .. } = &mut self.state else {
    //                 return Task::none();
    //             };

    //             match interaction {
    //                 chart::spectrogram::Interaction::ZoomChanged(zoom) => {
    //                     charts.spectrogram.zoom = zoom
    //                 }
    //                 chart::spectrogram::Interaction::OffsetChanged(offset) => {
    //                     charts.spectrogram.offset = offset
    //                 }
    //             }

    //             charts.spectrogram.cache.clear();

    //             Task::none()
    //         }
    //         Message::SpectrogramComputed(id, data) => {
    //             log::debug!(
    //                 "Spectrogram for measurement (ID: {}) with: {} slices, computed.",
    //                 id,
    //                 data.len()
    //             );

    //             let Some(spectrogram) = self
    //                 .measurements
    //                 .get_mut(id)
    //                 .and_then(ui::Measurement::analysis_mut)
    //                 .map(|analysis| &mut analysis.spectrogram)
    //             else {
    //                 return Task::none();
    //             };

    //             spectrogram.computed(data);

    //             if let State::Analysing { charts, .. } = &mut self.state {
    //                 charts.spectrogram.cache.clear();
    //             };

    //             Task::none()
    //         }
    //         Message::OpenSpectralDecayConfig => {
    //             self.modal = Modal::SpectralDecayConfig(SpectralDecayConfig::new(
    //                 self.spectral_decay_config,
    //             ));

    //             Task::none()
    //         }
    //         Message::SpectralDecayConfig(message) => {
    //             let Modal::SpectralDecayConfig(config) = &mut self.modal else {
    //                 return Task::none();
    //             };

    //             let State::Analysing {
    //                 selected: selected_analysis,
    //                 ref loopback,
    //                 ..
    //             } = self.state
    //             else {
    //                 return Task::none();
    //             };

    //             match config.update(message) {
    //                 Some(action) => {
    //                     self.modal = Modal::None;

    //                     if let spectral_decay_config::Action::Apply(config) = action {
    //                         self.spectral_decay_config = config;

    //                         self.measurements
    //                             .iter_mut()
    //                             .flat_map(ui::Measurement::analysis_mut)
    //                             .for_each(|analysis| {
    //                                 analysis.spectral_decay = ui::spectral_decay::State::default()
    //                             });

    //                         selected_analysis
    //                             .and_then(|id| self.measurements.iter_mut().find(|m| m.id() == id))
    //                             .map_or(Task::none(), |measurement| {
    //                                 compute_spectral_decay(
    //                                     loopback,
    //                                     measurement,
    //                                     self.spectral_decay_config,
    //                                 )
    //                             })
    //                     } else {
    //                         Task::none()
    //                     }
    //                 }
    //                 None => Task::none(),
    //             }
    //         }
    //         Message::OpenSpectrogramConfig => {
    //             self.modal =
    //                 Modal::SpectrogramConfig(SpectrogramConfig::new(self.spectrogram_config));

    //             Task::none()
    //         }
    //         Message::SpectrogramConfig(message) => {
    //             let Modal::SpectrogramConfig(config) = &mut self.modal else {
    //                 return Task::none();
    //             };

    //             let State::Analysing {
    //                 selected: selected_analysis,
    //                 ref loopback,
    //                 ..
    //             } = self.state
    //             else {
    //                 return Task::none();
    //             };

    //             match config.update(message) {
    //                 spectrogram_config::Action::None => Task::none(),
    //                 spectrogram_config::Action::Close => {
    //                     self.modal = Modal::None;

    //                     Task::none()
    //                 }
    //                 spectrogram_config::Action::ConfigChanged(preferences) => {
    //                     self.modal = Modal::None;
    //                     self.spectrogram_config = preferences;

    //                     self.measurements
    //                         .iter_mut()
    //                         .flat_map(ui::Measurement::analysis_mut)
    //                         .for_each(|analysis| {
    //                             analysis.spectrogram = ui::spectrogram::State::default()
    //                         });

    //                     selected_analysis
    //                         .and_then(|id| self.measurements.get_mut(id))
    //                         .map_or(Task::none(), |measurement| {
    //                             compute_spectrogram(loopback, measurement, &self.spectrogram_config)
    //                         })
    //                 }
    //             }
    //         }
    //     }
    // }

    pub fn view<'a>(&'a self, recent_projects: &'a RecentProjects) -> Element<'a, Message> {
        let header = {
            let project_menu = {
                let recent_project_entries = column(
                    recent_projects
                        .iter()
                        .enumerate()
                        .filter_map(|(i, p)| p.file_name().map(|f| (i, f)))
                        .filter_map(|(i, p)| p.to_str().map(|f| (i, f)))
                        .map(|(i, s)| {
                            button(s)
                                .on_press(Message::LoadRecentProject(i))
                                .style(button::subtle)
                                .width(Length::Fill)
                                .into()
                        }),
                )
                .width(Length::Fill);
                column![
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
                .width(Length::Fill)
            };

            let tab = |s, is_active, id: Option<_>| {
                button(text(s).size(20))
                    .padding(10)
                    .style(move |theme: &Theme, status| {
                        let palette = theme.extended_palette();

                        let base = button::text(theme, status);

                        if is_active {
                            button::Style {
                                text_color: palette.background.neutral.text,
                                background: Some(palette.background.base.color.into()),
                                ..base
                            }
                        } else {
                            base
                        }
                    })
                    .on_press_maybe(id.map(Message::OpenTab))
            };

            let active_tab = self.state.active_tab();
            let tabs = row![
                tab(
                    "Measurements",
                    active_tab.is_none() || matches!(active_tab, Some(Tab::Measurements { .. })),
                    Some(tab::Id::Measurements)
                ),
                tab(
                    "Impulse Responses",
                    matches!(active_tab, Some(Tab::ImpulseResponses { .. })),
                    active_tab.is_some().then_some(tab::Id::ImpulseResponses)
                ),
                tab(
                    "Frequency Responses",
                    matches!(active_tab, Some(Tab::FrequencyResponses { .. })),
                    active_tab.is_some().then_some(tab::Id::FrequencyResponses)
                ),
                tab(
                    "Spectral Decays",
                    matches!(active_tab, Some(Tab::SpectralDecays { .. })),
                    active_tab.is_some().then_some(tab::Id::SpectralDecays)
                ),
                tab(
                    "Spectrogram",
                    matches!(active_tab, Some(Tab::Spectrograms)),
                    active_tab.is_some().then_some(tab::Id::Spectrograms)
                ),
            ]
            .spacing(5)
            .align_y(Center);

            container(column![
                dropdown_root("Project", project_menu).style(button::secondary),
                tabs,
            ])
            .width(Length::Fill)
            .style(container::dark)
        };

        let content = {
            match self.state {
                State::Collecting => self.measurements_tab(),
                State::Analysing {
                    ref active_tab,
                    selected,
                    ref analyses,
                } => match active_tab {
                    Tab::Measurements { .. } => self.measurements_tab(),
                    Tab::ImpulseResponses {
                        pending_window: window_settings,
                    } => self.impulse_responses_tab(
                        selected,
                        &self.ir_chart,
                        window_settings,
                        analyses,
                    ),
                    Tab::FrequencyResponses { cache } => {
                        self.frequency_responses_tab(cache, analyses)
                    }
                    Tab::SpectralDecays { cache } => {
                        self.spectral_decay_tab(selected, &analyses, &cache)
                    }
                    Tab::Spectrograms => {
                        self.spectrogram_tab(selected, analyses, &self.spectrogram)
                    }
                },
            }
        };

        let content = if let Some(recording) = &self.recording {
            container(recording.view().map(Message::Recording)).padding(10)
        } else {
            container(column![header, container(content).padding(10)])
        };

        match self.modal {
            Modal::None => content.into(),
            Modal::PendingWindow { .. } => {
                modal(content, modal::pending_window().map(Message::PendingWindow))
            }
            Modal::SpectralDecayConfig(ref config) => {
                modal(content, config.view().map(Message::SpectralDecayConfig))
            }
            Modal::SpectrogramConfig(ref config) => {
                modal(content, config.view().map(Message::SpectrogramConfig))
            }
        }
    }
    // pub fn view<'a>(&'a self, recent_projects: &'a RecentProjects) -> Element<'a, Message> {
    //     let header = {
    //         let project_menu = {
    //             let recent_project_entries = column(
    //                 recent_projects
    //                     .iter()
    //                     .enumerate()
    //                     .filter_map(|(i, p)| p.file_name().map(|f| (i, f)))
    //                     .filter_map(|(i, p)| p.to_str().map(|f| (i, f)))
    //                     .map(|(i, s)| {
    //                         button(s)
    //                             .on_press(Message::RecentProject(i))
    //                             .style(button::subtle)
    //                             .width(Length::Fill)
    //                             .into()
    //                     }),
    //             )
    //             .width(Length::Fill);
    //             column![
    //                 button("New")
    //                     .on_press(Message::NewProject)
    //                     .style(button::subtle)
    //                     .width(Length::Fill),
    //                 button("Save")
    //                     .on_press(Message::SaveProject)
    //                     .style(button::subtle)
    //                     .width(Length::Fill),
    //                 button("Open ...")
    //                     .on_press(Message::LoadProject)
    //                     .style(button::subtle)
    //                     .width(Length::Fill),
    //                 dropdown_menu("Open recent ...", recent_project_entries)
    //                     .style(button::subtle)
    //                     .width(Length::Fill),
    //             ]
    //             .width(Length::Fill)
    //         };

    //         container(column![
    //             dropdown_root("Project", project_menu).style(button::secondary),
    //             match &self.state {
    //                 State::CollectingMeasuremnts { .. } => TabId::Measurements.view(false),
    //                 State::Analysing { active_tab, .. } => TabId::from(active_tab).view(true),
    //             }
    //         ])
    //         .width(Length::Fill)
    //         .style(container::dark)
    //     };

    //     let content = match &self.state {
    //         State::CollectingMeasuremnts {
    //             recording,
    //             loopback,
    //         } => {
    //             if let Some(recording) = recording {
    //                 recording.view().map(Message::Recording)
    //             } else {
    //                 self.measurements_tab(loopback.as_ref())
    //             }
    //         }
    //         State::Analysing {
    //             active_tab,
    //             selected: selected_analysis,
    //             charts,
    //             loopback,
    //             ..
    //         } => match active_tab {
    //             Tab::Measurements { recording } => {
    //                 if let Some(recording) = recording {
    //                     recording.view().map(Message::Recording)
    //                 } else {
    //                     self.measurements_tab(Some(loopback))
    //                 }
    //             }
    //             Tab::ImpulseResponses {
    //                 window_settings, ..
    //             } => self.impulse_responses_tab(
    //                 *selected_analysis,
    //                 &charts.impulse_responses,
    //                 window_settings,
    //             ),
    //             Tab::FrequencyResponses => {
    //                 self.frequency_responses_tab(&charts.frequency_responses)
    //             }
    //             Tab::SpectralDecay => {
    //                 self.spectral_decay_tab(*selected_analysis, &charts.spectral_decay_cache)
    //             }
    //             Tab::Spectrogram => self.spectrogram_tab(*selected_analysis, &charts.spectrogram),
    //         },
    //     };

    //     let content = container(column![header, container(content).padding(10)]);

    // }

    fn measurements_tab<'a>(&'a self) -> Element<'a, Message> {
        let sidebar = {
            let loopback = Category::new("Loopback")
                .push_button(sidebar::button(icon::plus()).on_press(Message::LoadLoopback))
                .push_button(
                    sidebar::button(icon::record())
                        .on_press(Message::StartRecording(recording::Kind::Loopback)),
                )
                .push_entry_maybe(self.loopback.as_ref().map(|loopback| {
                    let active = self.selected == Some(measurement::Selected::Loopback);
                    loopback.view(active).map(Message::Measurement)
                }));

            let measurements = Category::new("Measurements")
                .push_button(sidebar::button(icon::plus()).on_press(Message::LoadMeasurement))
                .push_button(
                    sidebar::button(icon::record())
                        .on_press(Message::StartRecording(recording::Kind::Measurement)),
                )
                .extend_entries(self.measurements.iter().map(|measurement| {
                    let active =
                        self.selected == Some(measurement::Selected::Measurement(measurement.id()));
                    measurement.view(active).map(Message::Measurement)
                }));

            container(scrollable(
                column![loopback, rule::horizontal(1), measurements]
                    .spacing(10)
                    .padding(10),
            ))
            .style(|theme| {
                container::rounded_box(theme)
                    .background(theme.extended_palette().background.weakest.color)
            })
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
                        .and_then(Loopback::loaded)
                        .map(AsRef::as_ref),
                    measurement::Selected::Measurement(id) => {
                        self.measurements.get(id).and_then(Measurement::signal)
                    }
                }) {
                chart::waveform(measurement, &self.signal_cache, self.zoom, self.offset)
                    .map(Message::MeasurementChart)
            } else {
                welcome_text(text("Select a signal to view its data."))
            };

            container(content).center(Length::Fill).into()
        };

        column!(row![
            container(sidebar).width(Length::FillPortion(2)),
            container(content).width(Length::FillPortion(5))
        ])
        .spacing(10)
        .into()
    }

    pub fn impulse_responses_tab<'a>(
        &'a self,
        selected: Option<measurement::Id>,
        chart: &'a impulse_response::Chart,
        window: &'a Window,
        analyses: &'a BTreeMap<measurement::Id, Analysis>,
    ) -> Element<'a, Message> {
        let sidebar = {
            let header = sidebar::header("Impulse Responses");

            let entries = self.measurements.iter().flat_map(|measurement| {
                let active = selected == Some(measurement.id());
                let signal = measurement.signal()?;
                let progress = analyses
                    .get(&measurement.id())
                    .map(|a| a.impulse_response.progress());

                let entry = ui::impulse_response::view(
                    &measurement.name,
                    signal.modified,
                    progress,
                    active,
                )
                .map(Message::ImpulseResponse.with(measurement.id()));

                Some(entry)
            });

            container(column![header, scrollable(column(entries))].spacing(6))
                .padding(6)
                .style(|theme| {
                    container::rounded_box(theme)
                        .background(theme.extended_palette().background.weakest.color)
                })
        };

        let content = {
            let placeholder = center(text("Impulse response not computed, yet."));

            selected
                .as_ref()
                .and_then(|id| analyses.get(id))
                .and_then(Analysis::impulse_response)
                .map(|impulse_response| {
                    chart
                        .view(impulse_response, window)
                        .map(Message::ImpulseResponseChart)
                })
                .unwrap_or(placeholder.into())
        };

        row![
            container(sidebar)
                .width(Length::FillPortion(2))
                .style(container::bordered_box),
            container(content).width(Length::FillPortion(5))
        ]
        .spacing(10)
        .into()
    }

    fn frequency_responses_tab<'a>(
        &'a self,
        cache: &'a canvas::Cache,
        analyses: &'a BTreeMap<measurement::Id, Analysis>,
    ) -> Element<'a, Message> {
        let sidebar = {
            let header = sidebar::header("Frequency Responses");

            let entries = self.measurements.iter().flat_map(|measurement| {
                let analysis = analyses.get(&measurement.id())?;

                let content = analysis.frequency_response.view(
                    &measurement.name,
                    Message::FrequencyResponseToggled.with(measurement.id()),
                );

                Some(content)
            });

            container(column![header, scrollable(column(entries).spacing(6))].spacing(6))
                .padding(6)
                .style(|theme| {
                    container::rounded_box(theme)
                        .background(theme.extended_palette().background.weakest.color)
                })
        };

        let header = {
            row![pick_list(
                frequency_response::Smoothing::ALL,
                Some(&self.smoothing),
                Message::ChangeSmoothing,
            )]
        };

        let frequency_responses = analyses.values().map(|a| &a.frequency_response);
        let chart_needed = frequency_responses
            .clone()
            .any(|fr| fr.result().is_some() && fr.is_shown);

        let content = if chart_needed {
            let series_list = frequency_responses
                .filter(|fr| fr.is_shown)
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
                .y_labels(Labels::default().format(&|v| format!("{v:.0}")))
                .extend_series(series_list)
                .cache(&cache);

            container(chart)
        } else {
            container(text("Please select a frequency respone.")).center(Length::Fill)
        };

        row![
            container(sidebar)
                .width(Length::FillPortion(2))
                .style(container::bordered_box),
            column![header, container(content).width(Length::FillPortion(5))].spacing(12)
        ]
        .spacing(10)
        .into()
    }

    pub fn spectral_decay_tab<'a>(
        &'a self,
        selected: Option<ui::measurement::Id>,
        analyses: &'a BTreeMap<measurement::Id, Analysis>,
        cache: &'a canvas::Cache,
    ) -> Element<'a, Message> {
        let sidebar = {
            let header = {
                let config_btn = button(icon::settings().center())
                    .style(button::subtle)
                    .on_press(Message::OpenSpectralDecayConfig);
                Category::new("Spectral Decays").push_button(config_btn)
            };

            let entries = self.measurements.iter().flat_map(|measurement| {
                let id = measurement.id();
                let is_active = selected.is_some_and(|s| s == id);

                let signal = measurement.signal()?;

                let entry = {
                    // TODO: refactor, basically the same btn as IR and Spectrogram
                    let dt: DateTime<Utc> = signal.modified.into();
                    let btn = button(
                        column![
                            text(&measurement.name)
                                .size(16)
                                .wrapping(text::Wrapping::WordOrGlyph),
                            text!("{}", dt.format("%x %X")).size(10)
                        ]
                        .clip(true)
                        .spacing(6),
                    )
                    // FIXME message type
                    .on_press_with(move || {
                        Message::ImpulseResponse(id, ui::impulse_response::Message::Select)
                    })
                    .width(Length::Fill)
                    .style(move |theme: &Theme, status| {
                        let base = button::subtle(theme, status);
                        let background = theme.extended_palette().background;

                        if is_active {
                            base.with_background(background.weak.color)
                        } else {
                            base
                        }
                    });

                    sidebar::item(btn, is_active)
                };

                // FIXME
                let analysis = analyses.get(&id);

                let entry = if let Some(analysis) = analysis {
                    match analysis.spectral_decay.progress() {
                        ui::spectral_decay::Progress::None => entry,
                        ui::spectral_decay::Progress::WaitingForImpulseResponse => {
                            processing_overlay("Impulse Response", entry)
                        }
                        ui::spectral_decay::Progress::Computing => {
                            processing_overlay("Computing ...", entry)
                        }
                        ui::spectral_decay::Progress::Finished => entry,
                    }
                } else {
                    entry
                };

                Some(entry)
            });

            container(column![header, scrollable(column(entries))].spacing(6))
                .padding(6)
                .style(|theme| {
                    container::rounded_box(theme)
                        .background(theme.extended_palette().background.weakest.color)
                })
        };

        let spectral_decay = selected
            .and_then(|id| analyses.get(&id))
            .map(|a| &a.spectral_decay);

        let content = if let Some(decay) = spectral_decay.and_then(ui::SpectralDecay::result) {
            let gradient = colorous::MAGMA;

            let series_list = decay.iter().enumerate().map(|(fr_index, fr)| {
                // FIXME refactor bin -> Hz computation
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
            center(text("Please select a frequency respone.").size(18))
        };

        row![
            container(sidebar)
                .width(Length::FillPortion(2))
                .style(container::bordered_box),
            container(content).width(Length::FillPortion(5))
        ]
        .spacing(10)
        .into()
    }

    fn spectrogram_tab<'a>(
        &'a self,
        selected: Option<measurement::Id>,
        analyses: &'a BTreeMap<measurement::Id, Analysis>,
        spectrogram: &'a Spectrogram,
    ) -> Element<'a, Message> {
        let sidebar = {
            let header = {
                let config_btn = button(icon::settings().center())
                    .style(button::subtle)
                    .on_press(Message::OpenSpectrogramConfig);
                Category::new("Spectrograms").push_button(config_btn)
            };

            let entries = self.measurements.iter().flat_map(|measurement| {
                let id = measurement.id();
                let is_active = selected.is_some_and(|selected| selected == id);

                let signal = measurement.signal()?;
                let entry = {
                    let dt: DateTime<Utc> = signal.modified.into();
                    let btn = button(
                        column![
                            text(&measurement.name)
                                .size(16)
                                .wrapping(text::Wrapping::WordOrGlyph),
                            text!("{}", dt.format("%x %X")).size(10)
                        ]
                        .clip(true)
                        .spacing(6),
                    )
                    .on_press_with(move || {
                        Message::ImpulseResponse(id, ui::impulse_response::Message::Select)
                    })
                    .width(Length::Fill)
                    .style(move |theme: &Theme, status| {
                        let base = button::subtle(theme, status);
                        let background = theme.extended_palette().background;

                        if is_active {
                            base.with_background(background.weak.color)
                        } else {
                            base
                        }
                    });

                    sidebar::item(btn, is_active)
                };

                let spectrogram = &analyses.get(&id).map(|a| &a.spectrogram);
                let entry = if let Some(spectrogram) = spectrogram {
                    match spectrogram.progress() {
                        ui::spectrogram::Progress::None => entry,
                        ui::spectrogram::Progress::ComputingImpulseResponse => {
                            processing_overlay("Impulse Response", entry)
                        }
                        ui::spectrogram::Progress::Computing => {
                            processing_overlay("Spectrogram", entry)
                        }
                        ui::spectrogram::Progress::Finished => entry,
                    }
                } else {
                    entry
                };

                Some(entry)
            });

            container(column![header, scrollable(column(entries))].spacing(6))
                .padding(6)
                .style(|theme| {
                    container::rounded_box(theme)
                        .background(theme.extended_palette().background.weakest.color)
                })
        };

        let spectrogram_data = selected
            .and_then(|id| analyses.get(&id))
            .and_then(|analysis| analysis.spectrogram.result());

        let content = if let Some(data) = spectrogram_data {
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
                .width(Length::FillPortion(2))
                .style(container::bordered_box),
            container(content).width(Length::FillPortion(5))
        ]
        .spacing(10)
        .into()
    }

    pub fn subscription(&self) -> Subscription<Message> {
        use keyboard::key;

        let hotkeys = keyboard::listen().filter_map(|event| match event {
            keyboard::Event::KeyPressed {
                key: keyboard::Key::Named(key),
                ..
            } => match key {
                key::Named::Shift => Some(Message::ShiftKeyPressed),
                _ => None?,
            },

            keyboard::Event::KeyReleased {
                key: keyboard::Key::Named(key),
                ..
            } => match key {
                key::Named::Shift => Some(Message::ShiftKeyReleased),
                _ => None?,
            },
            _ => None,
        });

        let recording = self
            .recording
            .as_ref()
            .map(Recording::subscription)
            .unwrap_or(Subscription::none());

        Subscription::batch([hotkeys, recording.map(Message::Recording)])
    }

    // fn analysing_possible(&self) -> bool {
    //     let is_loopback_loaded = if let State::CollectingMeasuremnts {
    //         loopback: Some(ref loopback),
    //         ..
    //     }
    //     | State::Analysing { ref loopback, .. } = self.state
    //     {
    //         loopback.is_loaded()
    //     } else {
    //         return false;
    //     };

    //     is_loopback_loaded && self.measurements.iter().any(ui::Measurement::is_loaded)
    // }
    // fn impulse_response_future(
    //     &mut self,
    //     id: measurement::Id,
    // ) -> Option<impl Future<Output = data::ImpulseResponse>> {
    //     let State::Analysing {
    //         ref mut analyses, ..
    //     } = self.state
    //     else {
    //         return None;
    //     };

    //     let analysis = analyses.entry(id).or_default();

    //     let Some(loopback) = self.loopback.as_ref().and_then(Loopback::loaded) else {
    //         return None;
    //     };

    //     let measurement = self
    //         .measurements
    //         .get(id)
    //         .and_then(Measurement::signal)
    //         .unwrap();

    //     analysis.compute_impulse_response(loopback.clone(), measurement.clone())
    // }
}

fn compute_impulse_response(
    analyses: &mut BTreeMap<measurement::Id, Analysis>,
    id: measurement::Id,
    loopback: Option<&Loopback>,
    measurements: &measurement::List,
) -> Task<Message> {
    let Some(loopback) = loopback.and_then(Loopback::loaded) else {
        return Task::none();
    };

    let Some(measurement) = measurements.get(id).and_then(Measurement::signal) else {
        return Task::none();
    };

    let analysis = analyses.entry(id).or_default();

    analysis
        .impulse_response
        .clone()
        .compute(loopback, measurement)
        .map(|sipper| {
            Task::sip(
                sipper,
                Message::ImpulseResponseComputed.with(id),
                Message::ImpulseResponseComputed.with(id),
            )
        })
        .unwrap_or_default()
}

fn compute_frequency_response(
    analyses: &mut BTreeMap<measurement::Id, Analysis>,
    id: measurement::Id,
    loopback: Option<&Loopback>,
    measurements: &measurement::List,
    window: data::Window<data::Samples>,
) -> Task<Message> {
    let analysis = analyses.entry(id).or_default();

    if analysis.frequency_response.result().is_some() {
        return Task::none();
    }

    if let Some(ir) = analysis.impulse_response.result() {
        // TODO move into analysis itself
        analysis.frequency_response.state = ui::frequency_response::State::Computing;
        Task::perform(
            data::frequency_response::compute(ir.data.clone(), window),
            Message::FrequencyResponseComputed.with(id),
        )
    } else {
        analysis.frequency_response.state =
            ui::frequency_response::State::WaitingForImpulseResponse;
        compute_impulse_response(analyses, id, loopback, measurements)
    }
}

fn compute_spectral_decay(
    id: measurement::Id,
    analyses: &mut BTreeMap<measurement::Id, Analysis>,
    config: data::spectral_decay::Config,
    loopback: Option<&Loopback>,
    measurements: &measurement::List,
) -> Task<Message> {
    let analysis = analyses.entry(id).or_default();

    if let Some(computation) = analysis
        .spectral_decay
        .compute(&analysis.impulse_response, config)
    {
        Task::perform(computation, Message::SpectralDecayComputed.with(id))
    } else {
        compute_impulse_response(analyses, id, loopback, measurements)
    }
}

fn compute_spectrogram(
    id: measurement::Id,
    analyses: &mut BTreeMap<measurement::Id, Analysis>,
    config: &spectrogram::Preferences,
    loopback: Option<&ui::Loopback>,
    measurements: &measurement::List,
) -> Task<Message> {
    let analysis = analyses.entry(id).or_default();

    if let Some(computation) = analysis
        .spectrogram
        .compute(&analysis.impulse_response, config)
    {
        Task::perform(computation, Message::SpectrogramComputed.with(id))
    } else {
        compute_impulse_response(analyses, id, loopback, measurements)
    }
}

impl Default for Main {
    fn default() -> Self {
        Self {
            state: State::default(),
            modal: Modal::None,
            recording: None,
            selected: None,

            loopback: None,
            measurements: measurement::List::default(),

            project_path: None,

            spectral_decay_config: data::spectral_decay::Config::default(),

            zoom: chart::Zoom::default(),
            offset: chart::Offset::default(),
            smoothing: frequency_response::Smoothing::default(),
            window: None,

            signal_cache: canvas::Cache::default(),

            ir_chart: impulse_response::Chart::default(),
            spectrogram: Spectrogram::default(),
            spectrogram_config: spectrogram::Preferences::default(),
        }
    }
}

async fn choose_impulse_response_file_path() -> Option<Arc<Path>> {
    rfd::AsyncFileDialog::new()
        .set_title("Save Impulse Response ...")
        // FIXME: remove
        .set_file_name("test.wave")
        .add_filter("wav", &["wav", "wave"])
        .add_filter("all", &["*"])
        .save_file()
        .await
        .as_ref()
        .map(|h| h.path().into())
}

// TODO: error handling
async fn save_impulse_response(path: Arc<Path>, ir: ui::ImpulseResponse) {
    tokio::task::spawn_blocking(move || {
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: ir.sample_rate.into(),
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        };

        let mut writer = hound::WavWriter::create(path, spec).unwrap();
        for s in ir.normalized {
            writer.write_sample(s).unwrap();
        }
        writer.finalize().unwrap();
    })
    .await
    .unwrap();
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
    .width(Length::Fill)
    .height(Length::Fill)
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
        let header = row![sidebar::header(self.title),]
            .padding(padding::right(6))
            .extend(self.buttons.into_iter().map(|btn| btn.into()))
            .spacing(6)
            .align_y(Alignment::Center);

        column!(header)
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

pub async fn pick_measurement_file(title: impl AsRef<str>) -> Option<PathBuf> {
    let handle = rfd::AsyncFileDialog::new()
        .set_title(title.as_ref())
        .add_filter("wav", &["wav", "wave"])
        .add_filter("all", &["*"])
        .pick_file()
        .await;

    handle.as_ref().map(FileHandle::path).map(Path::to_path_buf)
}

async fn pick_project_file_to_load() -> Option<PathBuf> {
    let handle = rfd::AsyncFileDialog::new()
        .set_title("Load project...")
        .add_filter("json", &["json"])
        .add_filter("all", &["*"])
        .pick_file()
        .await?;

    Some(handle.path().to_path_buf())
}

async fn pick_project_file_to_save() -> Option<PathBuf> {
    let handle = rfd::AsyncFileDialog::new()
        .set_title("Save project file ...")
        .save_file()
        .await?;

    Some(handle.path().to_path_buf())
}
