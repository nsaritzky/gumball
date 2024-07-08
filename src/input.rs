use sdl2::event::Event::{KeyDown, KeyUp};
use sdl2::keyboard::Keycode;

use crate::mmu::Mmu;

pub enum Button {
    A,
    B,
    Start,
    Select,
    Up,
    Down,
    Left,
    Right,
}

#[derive(Debug, Default)]
pub struct Input {
    select_button_keys: bool,
    select_direction_keys: bool,
    a: bool,
    b: bool,
    start: bool,
    select: bool,
    up: bool,
    down: bool,
    left: bool,
    right: bool,
}

impl Input {
    pub fn read_ff00(&self) -> u8 {
        let mut result = 0xCF; // Default value with bits 7-6 set to 1 and bits 5-4 as defined by selection

        // Update the result based on the button selection bits
        if self.select_button_keys {
            result &= !(1 << 5); // Clear bit 5 if button keys are selected
            if self.start {
                result &= !(1 << 3);
            }
            if self.select {
                result &= !(1 << 2);
            }
            if self.b {
                result &= !(1 << 1);
            }
            if self.a {
                result &= !(1 << 0);
            }
        }

        if self.select_direction_keys {
            result &= !(1 << 4); // Clear bit 4 if direction keys are selected
            if self.down {
                result &= !(1 << 3);
            }
            if self.up {
                result &= !(1 << 2);
            }
            if self.left {
                result &= !(1 << 1);
            }
            if self.right {
                result &= !(1 << 0);
            }
        }

        result
    }

    pub fn write_ff00(&mut self, value: u8) {
        // Only bits 5 and 4 are relevant for selecting the keys
        self.select_button_keys = value & (1 << 5) == 0;
        self.select_direction_keys = value & (1 << 4) == 0;
    }

    // Methods to update button states
    fn press_button(&mut self, button: Button) {
        match button {
            Button::A => self.a = true,
            Button::B => self.b = true,
            Button::Select => self.select = true,
            Button::Start => self.start = true,
            Button::Right => self.right = true,
            Button::Left => self.left = true,
            Button::Up => self.up = true,
            Button::Down => self.down = true,
        }
    }

    fn release_button(&mut self, button: Button) {
        match button {
            Button::A => self.a = false,
            Button::B => self.b = false,
            Button::Select => self.select = false,
            Button::Start => self.start = false,
            Button::Right => self.right = false,
            Button::Left => self.left = false,
            Button::Up => self.up = false,
            Button::Down => self.down = false,
        }
    }

    fn key_to_button(key: Keycode) -> Option<Button> {
        match key {
            Keycode::Z => Some(Button::A),
            Keycode::X => Some(Button::B),
            Keycode::Return => Some(Button::Start),
            Keycode::RShift => Some(Button::Select),
            Keycode::Up => Some(Button::Up),
            Keycode::Down => Some(Button::Down),
            Keycode::Left => Some(Button::Left),
            Keycode::Right => Some(Button::Right),
            _ => None,
        }
    }

    pub fn handle_event(&mut self, event: &sdl2::event::Event) {
        match event {
            KeyDown {
                keycode: Some(key), ..
            } => {
                if let Some(button) = Self::key_to_button(*key) {
                    self.press_button(button);
                }
            }
            KeyUp {
                keycode: Some(key), ..
            } => {
                if let Some(button) = Self::key_to_button(*key) {
                    self.release_button(button);
                }
            }
            _ => {}
        }
    }
}
