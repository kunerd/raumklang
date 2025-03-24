pub mod landing;
// pub mod measurements;

use iced::{
    widget::{container, text},
    Element, Length,
};
// use measurements::Measurements;
pub use landing::landing;

use std::ffi::OsStr;

// pub mod impulse_response;
// pub mod frequency_response;

// pub use frequency_response::FrequencyResponse;
// pub use impulse_response::ImpulseResponseTab;

pub enum Tab {
    Loading,
    Landing,
    Measurements,
}

pub fn loading<'a, Message: 'a>() -> Element<'a, Message> {
    container(text("Loading ...")).center(Length::Fill).into()
}

// async fn compute_impulse_response(
//     id: data::MeasurementId,
//     loopback: raumklang_core::Loopback,
//     measurement: raumklang_core::Measurement,
// ) -> (data::MeasurementId, raumklang_core::ImpulseResponse) {
//     let impulse_response = tokio::task::spawn_blocking(move || {
//         raumklang_core::ImpulseResponse::from_signals(&loopback, &measurement).unwrap()
//     })
//     .await
//     .unwrap();

//     (id, impulse_response)
// }
