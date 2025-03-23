use crate::OfflineMeasurement;

use super::{Loopback, Measurement, ProjectLoopback};

pub struct Project {
    pub loopback: Option<MeasurementState<Loopback, OfflineMeasurement>>,
    pub measurements: Vec<MeasurementState<Measurement, OfflineMeasurement>>,
}

#[derive(Debug)]
pub enum MeasurementState<L, O> {
    Loading(O),
    Loaded(L),
    NotLoaded(O),
}

impl Project {
    pub fn new(project: super::ProjectFile) -> Self {
        let loopback = project
            .loopback
            .map(OfflineMeasurement::from_loopback)
            .map(MeasurementState::Loading);

        let measurements = project
            .measurements
            .into_iter()
            .map(OfflineMeasurement::from_measurement)
            .map(MeasurementState::Loading)
            .collect();

        //                 let mut tasks = vec![];

        //                 recent_projects.insert(path);
        //                 let recent_projects = recent_projects.clone();
        //                 tasks.push(
        //                     Task::perform(
        //                         async move { save_recent_projects(&recent_projects).await },
        //                         |_| {},
        //                     )
        //                     .discard(),
        //                 );

        //                 *measurements = data::Store::new();
        //                 if let Some(loopback) = project.loopback {
        //                     let path = loopback.path().clone();
        //                     tasks.push(Task::perform(
        //                         async move {
        //                             load_signal_from_file(path.clone())
        //                                 .await
        //                                 .map(Arc::new)
        //                                 .map_err(|err| Error::File(path.to_path_buf(), Arc::new(err)))
        //                         },
        //                         Message::LoopbackMeasurementLoaded,
        //                     ));
        //                 }
        //                 for measurement in project.measurements {
        //                     let path = measurement.path.clone();
        //                     tasks.push(Task::perform(
        //                         async move {
        //                             load_signal_from_file(path.clone())
        //                                 .await
        //                                 .map(Arc::new)
        //                                 .map_err(|err| Error::File(path.to_path_buf(), Arc::new(err)))
        //                         },
        //                         Message::MeasurementLoaded,
        //                     ))
        //                 }

        //                 Task::batch(tasks)
        Self {
            loopback,
            measurements,
        }
    }
}
