use super::Component;

use crate::{
    audio,
    data::{self, measurement},
    widget::RmsPeakMeter,
};

use iced::{
    alignment::{Horizontal, Vertical},
    widget::{canvas, column, container, row, rule, text},
    Element, Length, Task,
};
use prism::{line_series, Chart};
use tokio_stream::wrappers::ReceiverStream;

#[derive(Debug)]
pub struct Measurement {
    sample_rate: data::SampleRate,
    finished_len: usize,
    loudness: audio::Loudness,
    data: Vec<f32>,
    cache: canvas::Cache,
}

#[derive(Debug, Clone)]
pub enum Message {
    RmsChanged(audio::Loudness),
    RecordingChunk(Box<[f32]>),
    RecordingFinished,
}

impl Measurement {
    pub fn new(config: measurement::Config, backend: &audio::Backend) -> (Self, Task<Message>) {
        let sample_rate = backend.sample_rate;
        let finished_len =
            data::Samples::from_duration(config.duration().into_inner(), sample_rate).into();

        let (loudness_receiver, mut data_receiver) = backend.run_measurement(config);

        let measurement_sipper = iced::task::sipper(async move |mut progress| {
            while let Some(data) = data_receiver.recv().await {
                progress.send(data).await;
            }
        });

        let measurement = Self {
            sample_rate,
            finished_len,
            loudness: audio::Loudness::default(),
            data: vec![],
            cache: canvas::Cache::new(),
        };

        let task = Task::batch(vec![
            Task::stream(ReceiverStream::new(loudness_receiver)).map(Message::RmsChanged),
            Task::sip(measurement_sipper, Message::RecordingChunk, |_| {
                Message::RecordingFinished
            }),
        ]);

        (measurement, task)
    }

    pub fn update(&mut self, message: Message) -> Option<raumklang_core::Measurement> {
        match message {
            Message::RecordingChunk(chunk) => {
                self.data.extend_from_slice(&chunk);
                None
            }
            Message::RecordingFinished => {
                let data = std::mem::replace(&mut self.data, Vec::new());

                Some(raumklang_core::Measurement::new(
                    self.sample_rate.into(),
                    data,
                ))
            }
            Message::RmsChanged(loudness) => {
                self.loudness = loudness;
                self.cache.clear();
                None
            }
        }
    }

    pub fn view(&self) -> Component<'_, Message> {
        Component::new("Measurement Running ...").content(
            row![
                container(
                    canvas(RmsPeakMeter::new(
                        self.loudness.rms,
                        self.loudness.peak,
                        &self.cache
                    ))
                    .width(60)
                    .height(200)
                )
                .padding(10),
                column![
                    container(
                        row![
                            loudness_text("RMS", self.loudness.rms),
                            rule::vertical(3),
                            loudness_text("Peak", self.loudness.peak),
                        ]
                        .align_y(Vertical::Bottom)
                        .height(Length::Shrink)
                        .spacing(10)
                    )
                    .center_x(Length::Fill),
                    Chart::<_, (), _>::new()
                        .x_range(0.0..=self.finished_len as f32)
                        .y_range(-0.5..=0.5)
                        .push_series(
                            line_series(self.data.iter().enumerate().map(|(i, s)| (i as f32, *s)))
                                .color(iced::Color::from_rgb8(50, 175, 50).scale_alpha(0.6))
                        )
                ]
                .spacing(12)
                .padding(10)
            ]
            .spacing(12)
            .align_y(Vertical::Center),
        )
    }
}

fn loudness_text<'a>(label: &'a str, value: f32) -> Element<'a, Message> {
    column![
        text(label).size(12).align_y(Vertical::Bottom),
        rule::horizontal(1),
        text!("{:.1}", value).size(24),
    ]
    .spacing(3)
    .width(Length::Shrink)
    .align_x(Horizontal::Center)
    .into()
}
