use iced::{mouse, widget::canvas, Color, Element, Length, Rectangle, Renderer, Theme};

#[derive(Debug)]
struct Circle {
    color: Color,
    radius: f32,
}

impl<Message> canvas::Program<Message> for Circle {
    type State = ();

    fn draw(
        &self,
        _state: &(),
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());

        let circle = canvas::Path::circle(frame.center(), self.radius);

        frame.fill(&circle, self.color);

        vec![frame.into_geometry()]
    }
}

pub fn colored_circle<'a, Message: 'a>(radius: f32, color: Color) -> Element<'a, Message> {
    canvas(Circle { radius, color })
        .width(Length::Fixed(radius * 2.0))
        .height(Length::Fixed(radius * 2.0))
        .into()
}
