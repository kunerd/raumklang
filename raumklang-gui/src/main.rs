mod tabs;
mod widgets;

use std::path::Path;

use iced::{Element, Font, Task};
use iced_aw::Tabs;

use tabs::{signals::WavLoadError, Tab};

#[derive(Debug, Default, Clone, PartialEq, Eq)]
enum TabId {
    #[default]
    Signals,
    ImpulseResponse,
}

#[derive(Default)]
struct State {
    signals: Signals,
    active_tab: TabId,
    signals_tab: tabs::Signals,
    impulse_response_tab: tabs::ImpulseResponse,
}

#[derive(Default)]
struct Signals {
    loopback: Option<Signal>,
    measurements: Vec<Signal>,
}

#[derive(Debug, Clone)]
enum Message {
    TabSelected(TabId),
    SignalsTab(tabs::signals::SignalsMessage),
    ImpulseResponse(tabs::impulse_resposne::Message),
}

impl State {
    fn title(&self) -> String {
        "Raumklang".to_owned()
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::TabSelected(id) => {
                self.active_tab = id;
                Task::none()
            }
            Message::SignalsTab(msg) => self
                .signals_tab
                .update(msg, &mut self.signals)
                .map(Message::SignalsTab),
            Message::ImpulseResponse(msg) => self
                .impulse_response_tab
                .update(msg, &self.signals)
                .map(Message::ImpulseResponse),
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let tabs = Tabs::new(Message::TabSelected)
            .push(
                TabId::Signals,
                self.signals_tab.label(),
                self.signals_tab
                    .view(&self.signals)
                    .map(Message::SignalsTab),
            )
            .push(
                TabId::ImpulseResponse,
                self.impulse_response_tab.label(),
                self.impulse_response_tab
                    .view(&self.signals)
                    .map(Message::ImpulseResponse),
            )
            .set_active_tab(&self.active_tab)
            .tab_bar_position(iced_aw::TabBarPosition::Top);

        tabs.into()
    }
}

#[derive(Debug, Clone)]
struct Signal {
    name: String,
    sample_rate: u32,
    data: Vec<f32>,
}

impl Signal {
    pub fn new(name: String, sample_rate: u32, data: Vec<f32>) -> Self {
        Self {
            name,
            sample_rate,
            data
        }
    }

    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, WavLoadError> {
        let name = path
            .as_ref()
            .file_name()
            .and_then(|n| n.to_os_string().into_string().ok())
            .unwrap_or("Unknown".to_string());

        let mut loopback = hound::WavReader::open(path).map_err(map_hound_error)?;
        let sample_rate = loopback.spec().sample_rate;
        // only mono files
        // currently only 32bit float
        let data = loopback
            .samples::<f32>()
            .collect::<hound::Result<Vec<f32>>>()
            .map_err(map_hound_error)?;

        Ok(Self {
            name,
            sample_rate,
            data,
        })
    }
}

fn map_hound_error(err: hound::Error) -> WavLoadError {
    match err {
        hound::Error::IoError(err) => WavLoadError::IoError(err.kind()),
        _ => WavLoadError::Other,
    }
}

fn main() -> iced::Result {
    iced::application(State::title, State::update, State::view)
        //.subscription(State::subscription)
        .default_font(Font::with_name("Noto Sans"))
        .antialiasing(true)
        .run()
}
