use crate::data;

use iced::{
    alignment::Vertical,
    widget::{
        button, column, container, horizontal_rule, horizontal_space, row, text,
        text::{Fragment, IntoFragment},
        Button,
    },
    Element, Length,
};

pub struct Page<'a, Message> {
    title: Fragment<'a>,
    sample_rate: Option<&'a data::SampleRate>,
    content: Option<Element<'a, Message>>,
    cancel_button: Element<'a, Message>,
    next_button: Element<'a, Message>,
}

impl<'a, Message> Page<'a, Message>
where
    Message: 'a + Clone,
{
    pub fn new(title: impl IntoFragment<'a>) -> Self {
        Self {
            title: title.into_fragment(),
            sample_rate: None,
            content: None,
            cancel_button: Button::new("Cancel").into(),
            next_button: Button::new("Next").into(),
        }
    }

    pub fn content(mut self, content: impl Into<Element<'a, Message>>) -> Self {
        self.content = Some(content.into());
        self
    }

    pub fn view(self) -> Element<'a, Message> {
        let header = |subsection| {
            column![
                row![
                    text!("Recording - {subsection}").size(20),
                    horizontal_space(),
                ]
                .push_maybe(
                    self.sample_rate
                        .map(|sample_rate| text!("Sample rate: {}", sample_rate).size(14))
                )
                .align_y(Vertical::Bottom),
                horizontal_rule(1),
            ]
            .spacing(4)
        };

        container(
            column![header(&self.title)]
                .push_maybe(self.content)
                .push(
                    container(row![self.cancel_button, self.next_button].spacing(6))
                        .align_right(Length::Fill),
                )
                .spacing(18),
        )
        .style(container::bordered_box)
        .padding(18)
        .into()
    }

    pub fn cancel_button(mut self, label: impl IntoFragment<'a>, message: Message) -> Self {
        self.cancel_button = button(text(label)).on_press(message).into();
        self
    }

    pub fn next_button(mut self, label: impl IntoFragment<'a>, message: Option<Message>) -> Self {
        self.next_button = button(text(label)).on_press_maybe(message).into();
        self
    }

    pub fn map<B>(self, f: impl Fn(Message) -> B + 'a + Clone) -> Page<'a, B>
    where
        B: 'a + Clone,
    {
        Page {
            title: self.title,
            sample_rate: self.sample_rate,
            content: self.content.map(|content| content.map(f.clone())),
            cancel_button: self.cancel_button.map(f.clone()),
            next_button: self.next_button.map(f),
        }
    }
}

impl<'a, Message> From<Page<'a, Message>> for Element<'a, Message>
where
    Message: 'a + Clone,
{
    fn from(page: Page<'a, Message>) -> Self {
        page.view()
    }
}
