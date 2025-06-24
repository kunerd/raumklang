pub trait Process {
    #[must_use]
    fn process(&mut self, data: &[f32]) -> Control;
}

pub enum Control {
    Continue,
    Stop,
}
