use iced::{
    executor,
    widget::{container, text},
    Application, Command, Element, Font, Settings, Subscription, Theme,
};

fn main() {
    State::run(Settings {
        antialiasing: true,
        default_font: Font::with_name("Noto Sans"),
        ..Settings::default()
    })
    .unwrap();
}

#[derive(Debug, Clone)]
enum Message {}

struct State {}

impl Application for State {
    type Message = self::Message;
    type Executor = executor::Default;
    type Flags = ();
    type Theme = Theme;

    fn new(_flags: Self::Flags) -> (Self, Command<Self::Message>) {
        let app = Self {};

        (app, Command::none())
    }

    fn title(&self) -> String {
        "Raumklang".to_owned()
    }

    fn update(&mut self, _message: Self::Message) -> Command<Self::Message> {
        Command::none()
    }

    fn view(&self) -> Element<'_, Self::Message> {
        container(text("Hello world".to_string())).into()
    }

    fn subscription(&self) -> Subscription<Self::Message> {
        //subscription::events()
        //    .map(TimeSeriesMessage::EventOccured)
        //    .map(ImpulseResponseMessage::TimeSeries)
        //    .map(Message::ImpulseRespone)
        Subscription::none()
    }
}
