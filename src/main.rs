mod apu;
mod background;
mod cpu;
mod debugger;
mod emulator;
mod input;
mod interrupts;
mod mmu;
mod ppu;
mod registers;
mod window;

use clap::Parser;

pub struct WindowCreator {
    canvas: sdl2::render::Canvas<sdl2::video::Window>,
    texture_creator: sdl2::render::TextureCreator<sdl2::video::WindowContext>,
}

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

fn main() {
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

    match rom {
        Ok(rom) => {
            mem.initialize_memory(rom);
            let emulator = emulator::Emulator::new(
                &mut main_window_creator.canvas,
                main_window_creator
                    .texture_creator
                    .create_texture_streaming(sdl2::pixels::PixelFormatEnum::RGB24, 160, 144)
                    .expect("Could not create texture"),
                mem,
                &audio_subsystem,
                event_pump,
                bg_window_creator,
                window_window,
            );
            let _ = emulator.and_then(|mut e| Ok(e.run(args.debug).map_err(|e| println!("{}", e))));
        }
        Err(e) => panic!("Error loading rom: {e}"),
    }
}
