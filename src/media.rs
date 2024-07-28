use crate::input::Button;

pub enum KeyEvent {
    Pressed(Option<Button>),
    Released(Option<Button>),
    Ignored,
}

pub trait Renderer {
    fn render(&mut self, pixel_buffer: &[u8]) -> Result<(), String>;
}

pub trait Event {
    fn to_key_event(&self) -> KeyEvent;
}

pub trait EventQueue {
    fn poll(&mut self) -> Vec<Box<dyn Event>>;
}
