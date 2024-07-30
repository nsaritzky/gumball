#[cfg(feature = "native")]
use std::time::{Duration, Instant};

// use crate::apu::APU;
use crate::cpu::Cpu;
use crate::input::Button;
use crate::interrupts::Interrupt;
use crate::media::{CrossPlatformError, Event, EventQueue, KeyEvent, Renderer};
use crate::mmu::Mmu;
use crate::ppu::PPU;

#[cfg(feature = "wasm")]
use crate::web::setup_web_keyboard_listener;

#[cfg(feature = "wasm")]
use web_sys::{console, js_sys};

const CLOCK_SPEED: u64 = 4_194_304;
const DIV_RATE: u64 = 16384;
const FRAME_DURATION: u64 = 16_743;

pub struct Emulator<'a> {
    cpu: Cpu,
    ppu: PPU<'a>,
    mmu: Mmu,
    // apu: AudioDevice<APU>,
    #[cfg(feature = "native")]
    event_queue: Box<dyn EventQueue>,
}

impl<'a> Emulator<'a> {
    pub fn new(
        renderer: impl Renderer + 'a,
        // audio_context: &'a sdl2::AudioSubsystem,
        #[cfg(feature = "native")] event_queue: impl EventQueue + 'static,
    ) -> Result<Self, CrossPlatformError> {
        // let desired_audio_spec = AudioSpecDesired {
        //     freq: Some(44100),
        //     channels: Some(1),
        //     samples: Some(512),
        // };
        // let audio_device = audio_context
        //     .open_playback(None, &desired_audio_spec, move |spec| APU::new(spec.freq))?;
        let mmu = Mmu::init();
        Ok(Self {
            cpu: Cpu::default(),
            ppu: PPU::new(renderer),
            mmu,
            // apu: audio_device,
            #[cfg(feature = "native")]
            event_queue: Box::new(event_queue),
        })
    }

    pub fn handle_event(&mut self, event: &dyn Event) {
        self.mmu.input.handle_event(event);
    }

    pub fn load_rom(&mut self, rom: Vec<u8>) {
        self.mmu.initialize_memory(rom);
    }

    #[cfg(feature = "native")]
    fn handle_event_poll(&mut self) {
        for event in self.event_queue.poll() {
            match event.to_key_event() {
                KeyEvent::Pressed(Some(Button::Quit)) => std::process::exit(0),
                KeyEvent::Pressed(..) | KeyEvent::Released(..) => {
                    self.mmu.input.handle_event(event.as_ref());
                }
                _ => {}
            }
        }
    }

    #[cfg(feature = "native")]
    pub fn run_native(&mut self, debug: bool) -> Result<(), CrossPlatformError> {
        let mut now = Instant::now();
        let mut timer_cycle_count = 0;
        let mut frame_time = Instant::now();
        let mut pause_at_frame = false;
        let mut new_frame = false;
        let mut first_frame_rendered = false;

        // self.apu.resume();

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

            // {
            //     let mut sound = self.apu.lock();
            //     sound.update(cycles as u32, &mut self.mmu);
            // }

            // self.cpu.log_state(&self.mmu);
            if self.ppu.render(&mut self.mmu, cycles as i32)? {
                // Only check for SDL events if the PPU rendered a frame
                new_frame = true;
                first_frame_rendered = true;

                self.handle_event_poll();

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
                // self.apu.lock().inc_div_apu(&self.mmu);
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

    #[cfg(feature = "wasm")]
    pub fn run_frame_wasm(&mut self) -> Result<(), CrossPlatformError> {
        let mut timer_cycle_count = 0;

        let mut cycles = 0;
        while !self.ppu.render(&mut self.mmu, cycles as i32)? {
            self.cpu.handle_interrupts(&mut self.mmu);
            self.cpu.enable_ime_delayed();

            if !self.cpu.halted && !self.cpu.stopped {
                cycles = self.cpu.execute(&mut self.mmu);
            } else {
                cycles = 4;
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
        }

        Ok(())
    }
}
