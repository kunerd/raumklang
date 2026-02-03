use std::fmt::{self};

use crate::data;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum Smoothing {
    #[default]
    None,
    OneOne,
    OneSecond,
    OneThird,
    OneSixth,
    OneTwelfth,
    OneTwentyFourth,
    OneFourtyEighth,
}

impl Smoothing {
    pub const ALL: [Smoothing; 8] = [
        Smoothing::None,
        Smoothing::OneOne,
        Smoothing::OneSecond,
        Smoothing::OneThird,
        Smoothing::OneSixth,
        Smoothing::OneTwelfth,
        Smoothing::OneTwentyFourth,
        Smoothing::OneFourtyEighth,
    ];

    pub fn fraction(&self) -> Option<u8> {
        match self {
            Smoothing::None => None,
            Smoothing::OneOne => Some(1),
            Smoothing::OneSecond => Some(2),
            Smoothing::OneThird => Some(3),
            Smoothing::OneSixth => Some(6),
            Smoothing::OneTwelfth => Some(12),
            Smoothing::OneTwentyFourth => Some(24),
            Smoothing::OneFourtyEighth => Some(48),
        }
    }
}

impl fmt::Display for Smoothing {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} smoothing",
            match self {
                Smoothing::None => "No",
                Smoothing::OneOne => "1/1",
                Smoothing::OneSecond => "1/2",
                Smoothing::OneThird => "1/3",
                Smoothing::OneSixth => "1/6",
                Smoothing::OneTwelfth => "1/12",
                Smoothing::OneTwentyFourth => "1/24",
                Smoothing::OneFourtyEighth => "1/48",
            }
        )
    }
}

pub async fn smooth_frequency_response(
    frequency_response: data::FrequencyResponse,
    fraction: u8,
) -> Box<[f32]> {
    tokio::task::spawn_blocking(move || {
        data::smooth_fractional_octave(&frequency_response.data.clone(), fraction)
    })
    .await
    .unwrap()
    .into_boxed_slice()
}
