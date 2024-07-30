#![cfg(feature = "wasm")]

use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use web_sys::{console, CanvasRenderingContext2d, ImageData, KeyboardEvent};
use web_sys::{js_sys, window};

use crate::input::{Button, Input};
use crate::media::CrossPlatformError;
use crate::media::{Event, EventQueue, KeyEvent, Renderer};

use crate::EMULATOR;

impl From<JsValue> for CrossPlatformError {
    fn from(js_value: JsValue) -> Self {
        CrossPlatformError::JsError(
            js_value
                .as_string()
                .unwrap_or_else(|| "Unknown JS error".to_string()),
        )
    }
}

impl From<CrossPlatformError> for JsValue {
    fn from(error: CrossPlatformError) -> Self {
        JsValue::from_str(&error.to_string())
    }
}

struct StrKeycode(String);

impl From<StrKeycode> for Option<Button> {
    fn from(keycode: StrKeycode) -> Self {
        match keycode.0.as_str() {
            "Escape" => Some(Button::Quit),
            "z" => Some(Button::A),
            "x" => Some(Button::B),
            "Enter" => Some(Button::Start),
            "Backspace" => Some(Button::Select),
            "ArrowUp" => Some(Button::Up),
            "ArrowDown" => Some(Button::Down),
            "ArrowRight" => Some(Button::Right),
            "ArrowLeft" => Some(Button::Left),
            _ => None,
        }
    }
}

pub struct WebRenderer(pub CanvasRenderingContext2d);

impl Event for KeyboardEvent {
    fn to_key_event(&self) -> KeyEvent {
        match self.type_().as_str() {
            "keydown" => KeyEvent::Pressed(StrKeycode(self.key()).into()),
            "keyup" => KeyEvent::Released(StrKeycode(self.key()).into()),
            _ => KeyEvent::Ignored,
        }
    }
}

impl Renderer for WebRenderer {
    fn render(&mut self, pixel_buffer: &[u8]) -> Result<(), CrossPlatformError> {
        let mut rgba_buffer = Vec::with_capacity(160 * 144 * 4);

        for rgb in pixel_buffer[0..160 * 144 * 3].chunks(3) {
            rgba_buffer.extend_from_slice(rgb); // Add R, G, B
            rgba_buffer.push(255); // Add full opacity Alpha channel
        }
        let image_data =
            ImageData::new_with_u8_clamped_array(wasm_bindgen::Clamped(&rgba_buffer), 160)
                .map_err(|e| {
                    console::log_1(&JsValue::from_str(&format!(
                        "Error creating image data: {:?}",
                        e
                    )));
                    CrossPlatformError::from(e)
                })?;
        self.0
            .put_image_data(&image_data, 0.0, 0.0)
            .map_err(|e| CrossPlatformError::from(e))
    }
}

#[cfg(feature = "wasm")]
#[wasm_bindgen]
pub fn setup_web_keyboard_listener() -> Result<(), CrossPlatformError>
where
{
    let window = window().expect("Could not get window object");
    let document = window.document().expect("Could not get document object");

    let closure = Closure::wrap(Box::new(move |event: KeyboardEvent| {
        event.prevent_default();
        EMULATOR.with(|emulator| {
            if let Some(emulator) = emulator.borrow_mut().as_mut() {
                emulator.handle_event(&event);
            }
        });
    }) as Box<dyn FnMut(KeyboardEvent)>);

    document.add_event_listener_with_callback("keydown", closure.as_ref().unchecked_ref())?;
    document.add_event_listener_with_callback("keyup", closure.as_ref().unchecked_ref())?;

    closure.forget();
    Ok(())
}
