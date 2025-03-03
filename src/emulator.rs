use sdl2::audio::{AudioDevice, AudioSpecDesired};
use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::render::Canvas;
use sdl2::video::Window;
use sdl2::EventPump;
use std::time::{Duration, Instant};

use crate::apu::{PulseChannel, APU};
use crate::background::BackgroundDisplay;
use crate::cpu::Cpu;
use crate::interrupts::Interrupt;
use crate::mmu::Mmu;
use crate::ppu::PPU;
use crate::window::WindowDisplay;
use crate::WindowCreator;

const CLOCK_SPEED: u64 = 4_194_304;
const DIV_RATE: u64 = 16384;
const FRAME_DURATION: u64 = 16_743;

pub struct Emulator<'a> {
    cpu: Cpu,
    ppu: PPU<'a>,
    mmu: Mmu,
    apu: AudioDevice<APU>,
    event_pump: sdl2::EventPump,
    background: Option<BackgroundDisplay>,
    window: Option<WindowDisplay>,
}

impl<'a> Emulator<'a> {
    pub fn new(
        canvas: &'a mut Canvas<Window>,
        texture: sdl2::render::Texture<'a>,
        mmu: Mmu,
        audio_context: &'a sdl2::AudioSubsystem,
        event_pump: EventPump,
        background_window_creator: Option<WindowCreator>,
        window_canvas: Option<Canvas<Window>>,
    ) -> Result<Self, String> {
        let desired_audio_spec = AudioSpecDesired {
            freq: Some(44100),
            channels: Some(1),
            samples: Some(512),
        };
        let audio_device = audio_context
            .open_playback(None, &desired_audio_spec, move |spec| APU::new(spec.freq))?;
        Ok(Self {
            cpu: Cpu::default(),
            ppu: PPU::new(canvas, texture)?,
            mmu,
            apu: audio_device,
            event_pump,
            background: background_window_creator.map(BackgroundDisplay::new),
            window: window_canvas.map(WindowDisplay::new),
        })
    }

    pub fn run(&mut self, debug: bool) -> Result<(), String> {
        let mut now = Instant::now();
        let mut timer_cycle_count = 0;
        let mut frame_time = Instant::now();
        let mut pause_at_frame = false;
        let mut new_frame = false;
        let mut first_frame_rendered = false;

        self.apu.resume();

        'running: loop {
            new_frame = false;
            let cycles;
            self.cpu.handle_interrupts(&mut self.mmu);
            self.cpu.enable_ime_delayed();

            if !self.cpu.halted && !self.cpu.stopped {
                cycles = self.cpu.execute(&mut self.mmu);
            } else {
                cycles = 4;
            }

            {
                let mut sound = self.apu.lock();
                sound.update(cycles as u32, &mut self.mmu);
            }

            // self.cpu.log_state(&self.mmu);
            if self.ppu.render(&mut self.mmu, cycles as i32)? {
                // Only check for SDL events if the PPU rendered a frame
                new_frame = true;
                first_frame_rendered = true;
                for event in self.event_pump.poll_iter() {
                    match event {
                        Event::Quit { .. }
                        | Event::KeyDown {
                            keycode: Some(Keycode::Escape),
                            ..
                        } => break 'running,
                        Event::KeyDown { .. } | Event::KeyUp { .. } => {
                            self.mmu.input.handle_event(&event);
                        }
                        _ => {}
                    }
                }
                if let Some(background) = &mut self.background {
                    background.draw_tiles(&self.mmu)?;
                }
                if let Some(window) = &mut self.window {
                    window.draw_tiles(&self.mmu)?;
                }
                let frame_elapsed = frame_time.elapsed();
                if frame_elapsed < Duration::from_micros(FRAME_DURATION) {
                    std::thread::sleep(Duration::from_micros(FRAME_DURATION) - frame_elapsed);
                } else {
                    // println!("Frame took too long: {:?}", frame_elapsed);
                }
                frame_time = Instant::now();
            }
            let mut time_elapsed = now.elapsed();
            while time_elapsed > Duration::from_nanos(1_000_000_000 / DIV_RATE) {
                self.mmu.inc_div();
                self.apu.lock().inc_div_apu(&self.mmu);
                time_elapsed -= Duration::from_nanos(1_000_000_000 / DIV_RATE);
                now = Instant::now();
            }

            let tac = self.mmu.get(0xFF07);
            let timer_enable = (tac & 0b100) >> 2 != 0;
            let timer_cycles = match tac & 0b11 {
                0b00 => 1024,
                0b01 => 16,
                0b10 => 64,
                0b11 => 256,
                _ => unreachable!(),
            };

            if timer_enable {
                timer_cycle_count += cycles;
                while timer_cycle_count >= timer_cycles {
                    timer_cycle_count -= timer_cycles;
                    let mut tima = self.mmu.get(0xFF05);
                    tima = tima.wrapping_add(1);
                    if tima == 0 {
                        self.mmu.set(0xFF05, self.mmu.get(0xFF06));
                        Interrupt::Timer.trigger(&mut self.mmu);
                    } else {
                        self.mmu.set(0xFF05, tima);
                    }
                }
            }

            // if self.mmu[0xFF01] != 0 {
            //     print!("{}", self.mmu[0xFF01] as char);
            //     self.mmu[0xFF01] = 0;
            // }
            if debug && first_frame_rendered {
                if pause_at_frame && !new_frame {
                    pause_at_frame = false;
                    continue;
                }

                self.cpu.log_state(&self.mmu);
                let mut input = String::new();
                std::io::stdin().read_line(&mut input).unwrap();
                match input.trim() {
                    "q" => break 'running,
                    "f" => {
                        pause_at_frame = true;
                    }
                    "s" => continue,
                    _ => {}
                }
            }
        }
        Ok(())
    }
}
