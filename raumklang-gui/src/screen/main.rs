mod chart;
mod frequency_response;
mod impulse_response;
mod modal;
mod recording;
mod tab;

use iced::Pixels;
use iced::mouse::ScrollDelta;
use iced_aksel::axis::{MarkerPosition, Position, TickContext, TickLine, TickResult};
use iced_aksel::scale;
use modal::Modal;
use tab::Tab;
use tokio::fs;

use crate::data::{
    self, Project, RecentProjects, SampleRate, Samples, Window, project, spectral_decay,
    spectrogram, window,
};
use crate::ui::frequency_response::SpectrumLayer;
use crate::{
    PickAndLoadError, icon, load_project, log,
    screen::main::{
        chart::waveform,
        modal::{
            SpectralDecayConfig, pending_window, save_project, spectral_decay_config,
            spectrogram_config,
        },
    },
    ui::{self, Analysis, Loopback, Measurement, measurement},
    widget::{processing_overlay, sidebar},
};

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
use rfd::FileHandle;

use std::io;
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
    measurement_operation: project::Operation,
    export_from_memory: bool,

    zoom: chart::Zoom,
    offset: chart::Offset,
    signal_cache: canvas::Cache,

    smoothing: frequency_response::Smoothing,
    window: Option<Window<Samples>>,

    ir_chart: impulse_response::Chart,
    spectrogram: Spectrogram,

    spectral_decay_config: spectral_decay::Config,
    spectrogram_config: spectrogram::Config,
    fr_state: iced_aksel::State<AxisId, f32>,
}

type AxisId = &'static str;

const FREQ_AXIS_ID: AxisId = "freq";
const DB_AXIS_ID: AxisId = "db";

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
    ProjectLoaded(Result<(Arc<Project>, PathBuf), PickAndLoadError>),
    SaveProject(PathBuf),
    OpenSaveProjectDialog,
    ProjectSaved(Result<(PathBuf, Project), ProjectError>),
    LoadRecentProject(usize),

    LoadLoopback,
    LoopbackLoaded(Loopback),
    LoadMeasurement,
    MeasurementLoaded(Measurement),
    Measurement(measurement::Message),

    OpenTab(tab::Id),
    ImpulseResponseComputed(measurement::Id, data::ImpulseResponse),
    SaveImpulseResponseToFile(measurement::Id, Option<Arc<Path>>),

    ImpulseResponseSaved(measurement::Id, Arc<Path>),
    ImpulseResponseChart(impulse_response::ChartOperation),
    ImpulseResponse(ui::measurement::Id, ui::impulse_response::Message),

    FrequencyResponseComputed(measurement::Id, data::FrequencyResponse),
    FrequencyResponseToggled(measurement::Id, bool),
    ChangeSmoothing(frequency_response::Smoothing),
    FrequencyResponseSmoothed(measurement::Id, Box<[f32]>),
    FrequencyResponseChart(frequency_response::Message),

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
    ProjectSaveDialog(save_project::Message),
}

impl Main {
    pub fn from_project(path: impl AsRef<Path>, project: Project) -> (Self, Task<Message>) {
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
                project_path: Some(path.as_ref().to_path_buf()),
                measurement_operation: project.measurement_operation,
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
            Message::OpenSaveProjectDialog => {
                self.modal = Modal::SaveProjectDialog(save_project::View::new(
                    self.measurement_operation,
                    self.export_from_memory,
                ));

                Task::none()
            }
            Message::ProjectSaveDialog(msg) => {
                let Modal::SaveProjectDialog(dialog) = &mut self.modal else {
                    return Task::none();
                };

                match dialog.update(msg) {
                    save_project::Action::None => Task::none(),
                    save_project::Action::Cancel => {
                        self.modal = Modal::None;
                        Task::none()
                    }
                    save_project::Action::Task(task) => task.map(Message::ProjectSaveDialog),
                    save_project::Action::Save(path_buf, operation, export_from_memory) => {
                        self.save_project(path_buf, operation, export_from_memory)
                    }
                }
            }
            Message::SaveProject(path) => {
                self.save_project(path, self.measurement_operation, self.export_from_memory)
            }
            Message::ProjectSaved(Ok((path, project))) => {
                // TODO: replace with soft-reload
                let (this, tasks) = Main::from_project(&path, project);
                *self = this;

                recent_projects.insert(path);

                let update_recent_projects = Task::future(recent_projects.clone().save()).discard();

                Task::batch([update_recent_projects, tasks])
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

                        Task::none()
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
                    Tab::Measurements => Task::none(),
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
                    Tab::Measurements => Task::none(),
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
                            frequency_response::smooth_frequency_response(
                                fr.origin.clone(),
                                fraction,
                            ),
                            Message::FrequencyResponseSmoothed.with(*id),
                        ))
                    });

                    Task::batch(tasks)
                } else {
                    analyses
                        .values_mut()
                        .map(Analysis::frequency_response_mut)
                        .for_each(|fr| fr.reset_smoothing());

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

                if let Some(data) = analyses
                    .get_mut(&id)
                    .map(|a| &mut a.frequency_response)
                    .and_then(ui::FrequencyResponse::result_mut)
                {
                    data.smoothed = Some(SpectrumLayer::new(
                        smoothed,
                        SampleRate::from(data.origin.sample_rate),
                    ));
                    cache.clear();
                }

                Task::none()
            }
            Message::FrequencyResponseChart(msg) => {
                match msg {
                    frequency_response::Message::OnPlotScroll(cursor_pos, delta) => match delta {
                        ScrollDelta::Lines { x: _, y } => {
                            let factor = 1.1f32.powf(y);

                            self.fr_state
                                .axis_mut(&FREQ_AXIS_ID)
                                .zoom(factor, Some(cursor_pos.x));
                            self.fr_state
                                .axis_mut(&DB_AXIS_ID)
                                .zoom(factor, Some(cursor_pos.y));
                        }
                        ScrollDelta::Pixels { x: _, y } => {
                            // For pixel-based scrolling (touchpad)
                            // Divide by larger number for less sensitive zooming
                            let factor = 1.0 + y / 500.0;

                            self.fr_state
                                .axis_mut(&FREQ_AXIS_ID)
                                .zoom(factor, Some(cursor_pos.x));
                            self.fr_state
                                .axis_mut(&DB_AXIS_ID)
                                .zoom(factor, Some(cursor_pos.y));
                        }
                    },
                    frequency_response::Message::OnPlotDrag(delta) => {
                        // --- Pan X-Axis ---
                        self.fr_state.axis_mut(&FREQ_AXIS_ID).pan(delta.x);
                        // self.clamp_x_axis();
                        self.fr_state.axis_mut(&DB_AXIS_ID).pan(delta.y);
                    }
                }
                // clamp
                let x_axis = self.fr_state.axis_mut(&FREQ_AXIS_ID);
                let (&min, &max) = x_axis.domain();
                x_axis.set_domain(min.max(MIN_FREQ), max.min(MAX_FREQ));

                let y_axis = self.fr_state.axis_mut(&DB_AXIS_ID);
                let (&min, &max) = y_axis.domain();
                y_axis.set_domain(min.max(MIN_DB), max.min(MAX_DB));

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
                    self.spectrogram_config.clone(),
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

                        let task = if let Some(id) = selected {
                            analyses
                                .get_mut(id)
                                .and_then(|a| {
                                    a.spectrogram.compute(&a.impulse_response, &preferences)
                                })
                                .map(|f| Task::perform(f, Message::SpectrogramComputed.with(*id)))
                                .unwrap_or_default()
                        } else {
                            Task::none()
                        };

                        self.spectrogram_config = preferences;
                        analyses.values_mut().for_each(|a| a.spectrogram.reset());

                        task
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
            Message::ProjectLoaded(Err(err)) => {
                log::error!("{err}");
                Task::none()
            }
            Message::ProjectSaved(Err(err)) => {
                log::error!("Could not save project to {:?} - {err}", self.project_path);
                Task::none()
            }
        }
    }

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
                        .on_press(if let Some(path) = self.project_path.as_ref() {
                            Message::SaveProject(path.clone())
                        } else {
                            Message::OpenSaveProjectDialog
                        })
                        .style(button::subtle)
                        .width(Length::Fill),
                    button("Save as ..")
                        .on_press(Message::OpenSaveProjectDialog)
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
                    active_tab.is_none() || matches!(active_tab, Some(Tab::Measurements)),
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
                    Tab::Measurements => self.measurements_tab(),
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
                        self.spectral_decay_tab(selected, analyses, cache)
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

        match &self.modal {
            Modal::None => content.into(),
            Modal::PendingWindow { .. } => {
                modal(content, modal::pending_window().map(Message::PendingWindow))
            }
            Modal::SpectralDecayConfig(config) => {
                modal(content, config.view().map(Message::SpectralDecayConfig))
            }
            Modal::SpectrogramConfig(config) => {
                modal(content, config.view().map(Message::SpectrogramConfig))
            }
            Modal::SaveProjectDialog(dialog) => {
                modal(content, dialog.view().map(Message::ProjectSaveDialog))
            }
        }
    }

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
                    measurement::Selected::Measurement(id) => self
                        .measurements
                        .get(id)
                        .and_then(Measurement::signal)
                        .map(AsRef::as_ref),
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
        _cache: &'a canvas::Cache,
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
            let chart = iced_aksel::Chart::new(&self.fr_state)
                .style(Box::new(|theme| {
                    let mut base = iced_aksel::style::default(theme);
                    let palette = theme.extended_palette();

                    base.axis.label.color = palette.secondary.base.color;
                    base.axis.tick.color = palette.secondary.base.color;
                    base.axis.spine.color = palette.secondary.base.color;
                    base.axis.grid.color = palette.background.weaker.color;

                    base
                }))
                .marker(&FREQ_AXIS_ID, MarkerPosition::Cursor, |ctx| {
                    Some(ctx.marker(format_frequency_label(ctx.value)))
                })
                .marker(&DB_AXIS_ID, MarkerPosition::Cursor, |ctx| {
                    Some(ctx.marker(format_db_label(ctx.value)))
                })
                .on_scroll(frequency_response::Message::OnPlotScroll)
                .on_drag(frequency_response::Message::OnPlotDrag);

            let chart = frequency_responses
                .filter(|fr| fr.is_shown)
                .fold(chart, |chart, fr| {
                    chart.plot_data(fr, FREQ_AXIS_ID, DB_AXIS_ID)
                });

            container(chart)
        } else {
            container(text("Please select a frequency respone.")).center(Length::Fill)
        };

        let content = Element::from(content).map(Message::FrequencyResponseChart);

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
        _cache: &'a canvas::Cache,
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

                let entry = if let Some(analysis) = analyses.get(&id) {
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

        let content = if let Some(decay) = spectral_decay {
            let chart = iced_aksel::Chart::new(&self.fr_state)
                .style(Box::new(|theme| {
                    let mut base = iced_aksel::style::default(theme);
                    let palette = theme.extended_palette();

                    base.axis.label.color = palette.secondary.base.color;
                    base.axis.tick.color = palette.secondary.base.color;
                    base.axis.spine.color = palette.secondary.base.color;
                    base.axis.grid.color = palette.background.weaker.color;

                    base
                }))
                .marker(&FREQ_AXIS_ID, MarkerPosition::Cursor, |ctx| {
                    Some(ctx.marker(format_frequency_label(ctx.value)))
                })
                .marker(&DB_AXIS_ID, MarkerPosition::Cursor, |ctx| {
                    Some(ctx.marker(format_db_label(ctx.value)))
                })
                .plot_data(decay, FREQ_AXIS_ID, DB_AXIS_ID);

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

    fn save_project(
        &self,
        path: PathBuf,
        measurement_operation: project::Operation,
        export_from_memory: bool,
    ) -> Task<Message> {
        let loopback = self.loopback.clone();
        let measurements: Vec<_> = self.measurements.iter().cloned().collect();

        Task::perform(
            save_project(
                path,
                loopback,
                measurements,
                export_from_memory,
                measurement_operation,
            ),
            Message::ProjectSaved,
        )
    }
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum ProjectError {
    #[error("not a sub-directory")]
    NoSubDirectory,
    #[error("dir is not empty: {0}")]
    Io(Arc<io::Error>),
}

impl From<io::Error> for ProjectError {
    fn from(err: io::Error) -> Self {
        ProjectError::Io(Arc::new(err))
    }
}

async fn save_project(
    path: impl AsRef<Path>,
    loopback: Option<Loopback>,
    measurements: impl IntoIterator<Item = Measurement>,
    export_from_memory: bool,
    measurement_operation: project::Operation,
) -> Result<(PathBuf, Project), ProjectError> {
    let path = path.as_ref();
    let project_dir = path.parent().ok_or(ProjectError::NoSubDirectory)?;

    fs::create_dir_all(&project_dir).await?;

    let loopback_path = if let Some(loopback) = loopback.as_ref() {
        if let Some(path) = loopback.path.as_ref() {
            Some(path.clone())
        } else if export_from_memory {
            let path = path.with_file_name("loopback.wav");
            loopback.clone().save(path).await
        } else {
            None
        }
    } else {
        None
    };

    let mut measurement_paths = vec![];
    for measurement in measurements {
        let path = if let Some(path) = measurement.path.as_ref() {
            Some(path.clone())
        } else if export_from_memory {
            let path = path.with_file_name(format!("measurement_{}.wav", measurement.id()));
            measurement.save(path).await
        } else {
            None
        };

        measurement_paths.extend(path);
    }

    let project = Project {
        loopback: loopback_path.map(project::Loopback::new),
        measurements: measurement_paths
            .into_iter()
            .map(project::Measurement::new)
            .collect(),
        measurement_operation,
        export_from_memory,
    };

    let project = project.save(path).await.unwrap();
    Ok((path.to_path_buf(), project))
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
    config: &spectrogram::Config,
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
        let mut fr_state = iced_aksel::State::new();

        fr_state.set_axis(FREQ_AXIS_ID, create_frequency_axis());
        fr_state.set_axis(DB_AXIS_ID, create_db_axis());

        Self {
            state: State::default(),
            modal: Modal::None,
            recording: None,
            selected: None,

            loopback: None,
            measurements: measurement::List::default(),

            project_path: None,
            measurement_operation: project::Operation::Copy,
            export_from_memory: true,

            spectral_decay_config: data::spectral_decay::Config::default(),

            zoom: chart::Zoom::default(),
            offset: chart::Offset::default(),
            smoothing: frequency_response::Smoothing::default(),
            window: None,

            signal_cache: canvas::Cache::default(),

            ir_chart: impulse_response::Chart::default(),
            spectrogram: Spectrogram::default(),
            spectrogram_config: spectrogram::Config::default(),

            fr_state,
        }
    }
}

const MIN_FREQ: f32 = 15.0;
const MAX_FREQ: f32 = 22_000.0;
const MIN_DB: f32 = -90.0;
const MAX_DB: f32 = 12.0;

fn create_frequency_axis() -> iced_aksel::Axis<f32> {
    iced_aksel::Axis::new(
        scale::Logarithmic::new(10.0, MIN_FREQ, MAX_FREQ),
        Position::Bottom,
    )
    .with_tick_renderer(frequency_tick_renderer)
    .skip_overlapping_labels(8.0)
}

fn create_db_axis() -> iced_aksel::Axis<f32> {
    iced_aksel::Axis::new(scale::Linear::new(MIN_DB, MAX_DB), Position::Left)
        .with_tick_renderer(db_tick_renderer)
        .with_thickness(80.0)
        .skip_overlapping_labels(8.0)
}

fn frequency_tick_renderer(ctx: TickContext<f32, Theme>) -> TickResult {
    let line = TickLine {
        length: Pixels(if ctx.tick.level == 0 { 12.0 } else { 6.0 }),
        ..ctx.tickline()
    };
    let label = format_frequency_label(ctx.tick.value);
    TickResult::with_label(ctx.label(label))
        .tick_line(line)
        .grid_line(ctx.gridline())
}

fn db_tick_renderer(ctx: TickContext<f32, Theme>) -> TickResult {
    let label = format_db_label(ctx.tick.value);
    TickResult::with_label(ctx.label(label))
        .tick_line(ctx.tickline())
        .grid_line(ctx.gridline())
}

fn format_frequency_label(value: f32) -> String {
    if value >= 10_000.0 {
        format!("{:.0} kHz", value / 1000.0)
    } else if value >= 1000.0 {
        format!("{:.1} kHz", value / 1000.0)
    } else {
        format!("{:.0} Hz", value)
    }
}

fn format_db_label(value: f32) -> String {
    format!("{:+.0} dB", value)
}

async fn choose_impulse_response_file_path() -> Option<Arc<Path>> {
    rfd::AsyncFileDialog::new()
        .set_title("Save Impulse Response ...")
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
