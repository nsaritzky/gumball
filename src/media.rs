use thiserror::Error;

use crate::input::Button;

#[derive(Error, Debug)]
pub enum CrossPlatformError {
    #[error("Native error: {0}")]
    NativeError(String),
    #[error("Js error: {0}")]
    JsError(String),
}

#[derive(Debug)]
pub enum KeyEvent {
    Pressed(Option<Button>),
    Released(Option<Button>),
    Ignored,
}

pub trait Renderer {
    fn render(&mut self, pixel_buffer: &[u8]) -> Result<(), CrossPlatformError>;
}

pub trait Event {
    fn to_key_event(&self) -> KeyEvent;
}

pub trait EventQueue {
    fn poll(&mut self) -> Vec<Box<dyn Event>>;
}
