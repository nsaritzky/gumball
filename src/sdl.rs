use std::cell::RefCell;
use std::rc::Rc;

use crate::input::Button;
use crate::media::{Event, EventQueue, KeyEvent, Renderer};
use sdl2::event::Event as SdlEvent;
use sdl2::keyboard::Keycode;
use sdl2::render::{Canvas as SdlCanvas, Texture};
use sdl2::video::Window;

struct SdlKeycode(Keycode);

impl From<SdlKeycode> for Option<Button> {
    fn from(keycode: SdlKeycode) -> Self {
        match keycode.0 {
            Keycode::Escape => Some(Button::Quit),
            Keycode::Z => Some(Button::A),
            Keycode::X => Some(Button::B),
            Keycode::Return => Some(Button::Start),
            Keycode::Backspace => Some(Button::Select),
            Keycode::Up => Some(Button::Up),
            Keycode::Down => Some(Button::Down),
            Keycode::Left => Some(Button::Left),
            Keycode::Right => Some(Button::Right),
            _ => None,
        }
    }
}

impl Event for SdlEvent {
    fn to_key_event(&self) -> KeyEvent {
        match self {
            SdlEvent::KeyDown {
                keycode: Some(keycode),
                ..
            } => KeyEvent::Pressed(SdlKeycode(*keycode).into()),
            SdlEvent::KeyUp {
                keycode: Some(keycode),
                ..
            } => KeyEvent::Released(SdlKeycode(*keycode).into()),
            _ => KeyEvent::Ignored,
        }
    }
}

impl EventQueue for sdl2::EventPump {
    fn poll(&mut self) -> Vec<Box<dyn Event>> {
        self.poll_iter()
            .map(|e| Box::new(e) as Box<dyn Event>)
            .collect()
    }
}

pub struct SdlRenderer<'a>(pub Texture<'a>, pub Rc<RefCell<SdlCanvas<Window>>>);

impl<'a> Renderer for SdlRenderer<'a> {
    fn render(&mut self, buffer: &[u8]) -> Result<(), String> {
        self.0
            .update(None, buffer, 160 * 3)
            .map_err(|e| e.to_string())?;
        self.1.borrow_mut().copy(&self.0, None, None)?;
        self.1.borrow_mut().present();
        Ok(())
    }
}
