mod tabs;
mod widgets;

use std::path::Path;

use iced::{
    widget::{column, text},
    Element, Font, Task,
};
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
    active_tab: TabId,
    signals_tab: tabs::Signals,
    impulse_response_tab: tabs::ImpulseResponse,
}

//impl State {
//    fn new() -> (Self, Task<Message>) {
//        (
//            Self {
//                active_tab: TabId::Signals,
//                signals_tab: tabs::Signals::
//                loopback_signal: None,
//                measurement_signal: None,
//                chart: None,
//            },
//            Task::none(),
//        )
//    }
//}

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
            Message::SignalsTab(msg) => self.signals_tab.update(msg).map(Message::SignalsTab),
            Message::ImpulseResponse(_) => todo!(),
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let tabs = Tabs::new(Message::TabSelected)
            .push(
                TabId::Signals,
                self.signals_tab.label(),
                self.signals_tab.view().map(Message::SignalsTab),
            )
            .push(
                TabId::ImpulseResponse,
                self.impulse_response_tab.label(),
                self.impulse_response_tab
                    .view()
                    .map(Message::ImpulseResponse),
            )
            .set_active_tab(&self.active_tab)
            //.tab_bar_style(style_from_index(theme))
            //.icon_font(ICON)
            .tab_bar_position(iced_aw::TabBarPosition::Top);

        tabs.into()
        //let side_menu = {
        //    let loopback_entry = {
        //        let header = text("Loopback");
        //        let btn = if let Some(signal) = &self.loopback_signal {
        //            button(signal.view()).on_press(Message::LoopbackSignalSelected)
        //        } else {
        //            button(text("load ...".to_string())).on_press(Message::LoadLoopbackSignal)
        //        }
        //        .style(button::secondary);

        //        column!(header, btn).width(Length::Fill).spacing(5)
        //    };

        //    let measurement_entry = {
        //        let header = text("Measurements");
        //        let btn = if let Some(signal) = &self.measurement_signal {
        //            button(signal.view()).on_press(Message::MeasurementSignalSelected)
        //        } else {
        //            button(text("load ...".to_string())).on_press(Message::LoadMeasurementSignal)
        //        }
        //        .style(button::secondary);

        //        column!(header, btn).width(Length::Fill).spacing(5)
        //    };

        //    container(column!(loopback_entry, measurement_entry).spacing(10))
        //        .padding(5)
        //        .width(Length::FillPortion(1))
        //};

        //let right_container = Tabs::new(Message::TabSelected)
        //    //.tab_icon_position(iced_aw::tabs::Position::Bottom)
        //    .on_close(Message::TabClosed)
        //    .push(
        //        TabId::Measurements,
        //        iced_aw::TabLabel::Text("Measurements".to_string()),
        //        {
        //            if let Some(chart) = &self.chart {
        //                container(chart.view().map(Message::TimeSeriesChart))
        //                    .width(Length::FillPortion(5))
        //            } else {
        //                container(text("Not implemented.".to_string()))
        //            }
        //        },
        //    )
        //    .push(
        //        TabId::RoomResponse,
        //        iced_aw::TabLabel::Text("Room impulse response".to_string()),
        //        text("Not implemented"),
        //    )
        //    .set_active_tab(&self.active_tab)
        //    //.tab_bar_style(style_from_index(theme))
        //    //.icon_font(ICON)
        //    .tab_bar_position(iced_aw::TabBarPosition::Top);

        //let right_container = right_container.width(Length::FillPortion(4));

        //let content = row!(side_menu, right_container);
        //content.into()
    }
}

#[derive(Debug, Clone)]
struct Signal {
    name: String,
    sample_rate: u32,
    data: Vec<f32>,
}

impl Signal {
    pub fn view(&self) -> Element<tabs::signals::SignalsMessage> {
        let samples = self.data.len();
        let sample_rate = self.sample_rate as f32;
        column!(
            text(&self.name),
            text(format!("Samples: {}", samples)),
            text(format!("Duration: {} s", samples as f32 / sample_rate)),
        )
        .padding(2)
        .into()
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
