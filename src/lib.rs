use crate::emulator::Emulator;
use crate::input::Input;
use crate::media::{CrossPlatformError, Event};
use crate::mmu::Mmu;
#[cfg(feature = "wasm")]
use crate::web::{setup_web_keyboard_listener, WebRenderer};
use clap::Parser;
use once_cell::sync::Lazy;
use std::{cell::RefCell, rc::Rc};
#[cfg(feature = "wasm")]
use wasm_bindgen::prelude::*;
#[cfg(feature = "wasm")]
use web_sys::{
    console, js_sys, window, CanvasRenderingContext2d, HtmlCanvasElement, KeyboardEvent, Window,
};

extern crate console_error_panic_hook;
use std::panic;

mod cpu;
mod debugger;
mod emulator;
mod input;
mod interrupts;
mod media;
mod mmu;
mod ppu;
mod registers;
mod web;

#[cfg(feature = "native")]
mod apu;
#[cfg(feature = "native")]
mod background;
#[cfg(feature = "native")]
mod sdl;
#[cfg(feature = "native")]
mod window;
#[cfg(feature = "native")]
use crate::sdl::SdlRenderer;

#[cfg(feature = "native")]
pub struct WindowCreator {
    canvas: sdl2::render::Canvas<sdl2::video::Window>,
    texture_creator: sdl2::render::TextureCreator<sdl2::video::WindowContext>,
}

#[cfg(feature = "wasm")]
thread_local! {
    pub static EMULATOR: Lazy<RefCell<Option<Emulator<'static>>>> = Lazy::new(|| RefCell::new(None));
}

#[cfg(feature = "native")]
impl WindowCreator {
    pub fn new(window: sdl2::video::Window) -> Self {
        let canvas = window
            .into_canvas()
            .build()
            .expect("Could not make a canvas");
        let texture_creator = canvas.texture_creator();
        Self {
            canvas,
            texture_creator,
        }
    }
}

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    rom_path: String,
    #[arg(short, long)]
    background: bool,
    #[arg(short, long)]
    debug: bool,
    #[arg(short, long)]
    window: bool,
}

#[cfg(feature = "native")]
pub fn native_main() {
    let args = Args::parse();
    let rom = mmu::load_rom(&args.rom_path);
    let mut mem = mmu::Mmu::init();

    let sdl_context = sdl2::init().unwrap();
    let video_subsystem = sdl_context
        .video()
        .expect("Could not initialize video subsystem");
    let audio_subsystem = sdl_context
        .audio()
        .expect("Could not initialize audio subsystem");
    let main_window = video_subsystem
        .window("Gumball", 160, 144)
        .position_centered()
        .build()
        .expect("Could not initialize video subsystem");

    // let background_window = args.background.then(|| {
    //     video_subsystem
    //         .window("Background", 256, 256)
    //         .position_centered()
    //         .build()
    //         .expect("Could not initialize video subsystem")
    //         .into_canvas()
    //         .build()
    //         .expect("Could not make a canvas for the background display")
    // });

    // let bg_texture_creator = background_window.map(|c| c.texture_creator());

    let mut main_window_creator = WindowCreator::new(main_window);

    let bg_window_creator = args.background.then(|| {
        WindowCreator::new(
            video_subsystem
                .window("Background", 256, 256)
                .position_centered()
                .build()
                .expect("Could not initialize video subsystem"),
        )
    });

    let window_window = args.window.then(|| {
        video_subsystem
            .window("Window", 256, 256)
            .position_centered()
            .build()
            .expect("Could not initialize video subsystem")
            .into_canvas()
            .build()
            .expect("Could not make a canvas for the background display")
    });

    let event_pump = sdl_context.event_pump().unwrap();

    let renderer = SdlRenderer(
        main_window_creator
            .texture_creator
            .create_texture_streaming(sdl2::pixels::PixelFormatEnum::RGB24, 160, 144)
            .expect("Could not create texture"),
        Rc::new(RefCell::new(main_window_creator.canvas)),
    );

    match rom {
        Ok(rom) => {
            let emulator = emulator::Emulator::new(renderer, event_pump);
            let _ = emulator.and_then(|mut e| {
                e.load_rom(rom);
                Ok(e.run_native(args.debug).map_err(|e| println!("{}", e)))
            });
        }
        Err(e) => panic!("Error loading rom: {e}"),
    }
}

#[cfg(feature = "wasm")]
#[wasm_bindgen]
pub fn setup_context(window: Window) -> Result<CanvasRenderingContext2d, JsValue> {
    let document = window.document().unwrap();
    let canvas = document
        .get_element_by_id("game-canvas")
        .expect("Could not find canvas element")
        .dyn_into::<HtmlCanvasElement>()?;
    let context = canvas
        .get_context("2d")?
        .expect("Could not get 2d context")
        .dyn_into::<CanvasRenderingContext2d>()?;

    Ok(context)
}

#[cfg(feature = "wasm")]
#[wasm_bindgen]
pub fn load_rom_and_run(rom: Vec<u8>) {
    EMULATOR.with(|emulator| {
        if let Some(emulator) = emulator.borrow_mut().as_mut() {
            emulator.load_rom(rom);
        }
    });

    let f = Rc::new(RefCell::new(None));
    let g = f.clone();

    let window = window().expect("Could not get window object");
    let performance = window
        .performance()
        .expect("Could not get performance object");

    let mut last_frame_time = performance.now();
    let frame_interval = 1000.0 / 60.0;

    *g.borrow_mut() = Some(Closure::wrap(Box::new(move || {
        let current_time = performance.now();
        let elapsed_time = current_time - last_frame_time;

        if elapsed_time >= frame_interval {
            EMULATOR.with(|emulator| {
                if let Some(emulator) = emulator.borrow_mut().as_mut() {
                    emulator.run_frame_wasm();
                }
            });

            last_frame_time = current_time - (elapsed_time % frame_interval);
        }
        request_animation_frame(f.borrow().as_ref().unwrap());
    }) as Box<dyn FnMut()>));

    request_animation_frame(g.borrow().as_ref().unwrap());
}

#[cfg(feature = "wasm")]
fn request_animation_frame(f: &Closure<dyn FnMut()>) {
    window()
        .expect("Could not get window object")
        .request_animation_frame(f.as_ref().unchecked_ref())
        .expect("Could not request animation frame");
}

#[cfg(feature = "wasm")]
#[wasm_bindgen(start)]
pub fn wasm_main() -> Result<(), JsValue> {
    panic::set_hook(Box::new(console_error_panic_hook::hook));
    let window = window().expect("Could not get window object");
    let context = setup_context(window)?;
    let renderer = WebRenderer(context);
    EMULATOR.with(|emulator| {
        let mut emulator = emulator.borrow_mut();
        *emulator = Some(Emulator::new(renderer).expect("Could not create emulator"));
    });
    Ok(())
}
