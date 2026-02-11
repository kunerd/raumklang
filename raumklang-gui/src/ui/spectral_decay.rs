use iced_aksel::{Measure, Plot, PlotData, Stroke, shape};

use crate::{
    data::{self, SampleRate},
    ui::{frequency_response::SpectrumLayer, impulse_response},
};

use std::future::Future;

#[derive(Debug, Clone, Default)]
pub struct SpectralDecay(State);

#[derive(Debug, Clone, Default)]
enum State {
    #[default]
    None,
    WaitingForImpulseResponse,
    Computing,
    Computed(Vec<SpectrumLayer>),
}

impl SpectralDecay {
    pub fn result(&self) -> Option<&Vec<SpectrumLayer>> {
        let State::Computed(result) = &self.0 else {
            return None;
        };

        Some(result)
    }
    pub fn progress(&self) -> Progress {
        match self.0 {
            State::None => Progress::None,
            State::WaitingForImpulseResponse => Progress::WaitingForImpulseResponse,
            State::Computing => Progress::Computing,
            State::Computed(_) => Progress::Finished,
        }
    }

    pub fn compute(
        &mut self,
        impulse_response: &impulse_response::State,
        config: data::spectral_decay::Config,
    ) -> Option<impl Future<Output = data::SpectralDecay> + use<>> {
        if self.result().is_some() {
            return None;
        }

        if let Some(impulse_response) = impulse_response.result() {
            self.0 = State::Computing;

            let computation = data::spectral_decay::compute(impulse_response.data.clone(), config);

            Some(computation)
        } else {
            self.0 = State::WaitingForImpulseResponse;
            None
        }
    }

    pub fn set_result(&mut self, spectral_decay: data::SpectralDecay) {
        let spectral_decay = spectral_decay
            .into_iter()
            .map(|fr| SpectrumLayer::new(fr.data.iter().copied(), SampleRate::from(fr.sample_rate)))
            .collect();

        self.0 = State::Computed(spectral_decay);
    }

    pub fn reset(&mut self) {
        self.0 = State::None
    }
}

#[derive(Debug, Clone)]
pub enum Progress {
    None,
    WaitingForImpulseResponse,
    Computing,
    Finished,
}

impl PlotData<f32> for SpectralDecay {
    fn draw(&self, plot: &mut Plot<f32>, _theme: &iced::Theme) {
        let State::Computed(ref sd) = self.0 else {
            return;
        };

        if sd.len() < 2 {
            return;
        }

        let gradient = colorous::MAGMA;
        for (i, fr) in sd.iter().enumerate() {
            let color = gradient.eval_rational(i, sd.len());
            let color = iced::Color::from_rgb8(color.r, color.g, color.b);

            let line_stroke = Stroke::new(color.scale_alpha(0.8), Measure::Screen(1.0));
            plot.add_shape(shape::Polyline::new(fr.0.clone()).stroke(line_stroke));
        }
    }
}
