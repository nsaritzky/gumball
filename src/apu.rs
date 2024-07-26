use sdl2::audio::{AudioCallback, AudioFormatNum};
use std::{
    cmp::min,
    sync::{Arc, Mutex},
};

use crate::mmu::Mmu;
use crate::registers::*;

const CPU_CLOCK_SPEED: u32 = 1_048_576;
const FADE_DURATION: f32 = 0.0;

pub struct APU {
    clock_cycles: u32,
    div_apu: u32,
    last_div: u8,
    buffer: Arc<Mutex<Vec<f32>>>,
    position: usize,
    sample_rate: i32,
    pulse_channel_1: PulseChannel,
    pulse_channel_2: PulseChannel,
    wave_channel: WaveChannel,
}

impl AudioCallback for APU {
    type Channel = f32;

    fn callback(&mut self, out: &mut [f32]) {
        let mut buffer = self.buffer.lock().unwrap();
        for x in out.iter_mut() {
            if self.position < buffer.len() {
                *x = buffer[self.position];
                self.position += 1;
            } else {
                *x = Self::Channel::SILENCE;
            }
        }

        if self.position > 0 {
            buffer.drain(0..self.position);
            self.position = 0;
        }
    }
}

impl APU {
    pub fn new(sample_rate: i32) -> Self {
        APU {
            clock_cycles: 0,
            div_apu: 0,
            last_div: 0,
            buffer: Arc::new(Mutex::new(Vec::new())),
            position: 0,
            sample_rate,
            pulse_channel_1: PulseChannel::new(
                1,
                sample_rate,
                Some(0xFF10),
                0xFF11,
                0xFF12,
                0xFF13,
                0xFF14,
            ),
            pulse_channel_2: PulseChannel::new(
                2,
                sample_rate,
                None,
                0xFF16,
                0xFF17,
                0xFF18,
                0xFF19,
            ),
            wave_channel: WaveChannel::new(sample_rate),
        }
    }

    pub fn update(&mut self, cycles: u32, mmu: &mut Mmu) {
        self.clock_cycles += cycles;
        let num_samples =
            self.clock_cycles as f32 / ((CPU_CLOCK_SPEED / self.sample_rate as u32) as f32);
        if num_samples < 1.0 {
            return;
        }
        let mut buffer = self.buffer.lock().unwrap();
        for _ in 0..num_samples as usize {
            if buffer.len() < self.sample_rate as usize / 10 {
                let sample1 = self.pulse_channel_1.generate_sample(mmu);
                let sample2 = self.pulse_channel_2.generate_sample(mmu);
                let sample3 = self.wave_channel.generate_sample(mmu);
                buffer.push((sample1 + sample2 + sample3) / 3.0);
            }
        }
        self.clock_cycles -=
            (num_samples as f32 * (CPU_CLOCK_SPEED / self.sample_rate as u32) as f32) as u32;
    }

    pub fn inc_div_apu(&mut self, mmu: &Mmu) {
        if self.last_div & 0x10 == 0x10 && mmu.get(0xFF04) & 0x10 == 0 {
            self.div_apu = self.div_apu.wrapping_add(1);
            self.pulse_channel_1.div_apu = self.pulse_channel_1.div_apu.wrapping_add(1);
            self.pulse_channel_2.div_apu = self.pulse_channel_2.div_apu.wrapping_add(1);
        }
        self.last_div = mmu.get(0xFF04);
    }
}

#[derive(Debug, Default)]
struct SquareWaveChannel {
    frequency: f32,   // Frequency of the square wave in Hz
    duty_cycle: f32,  // Duty cycle (fraction of the period the wave is high)
    sample_rate: f32, // Sample rate of the audio
    phase: f32,       // Current phase of the wave
    amplitude: u8,    // Amplitude of the wave
    fade_out_samples: u32,
    fade_in_samples: u32,
}

impl SquareWaveChannel {
    fn new(sample_rate: f32) -> Self {
        Self {
            frequency: 0.0,
            duty_cycle: 0.5,
            sample_rate,
            phase: 0.0,
            amplitude: 0,
            fade_out_samples: 0,
            fade_in_samples: 0,
        }
    }

    fn generate_sample(&mut self) -> f32 {
        // Calculate the period of the wave
        let period = self.sample_rate / self.frequency;

        // Calculate the current sample value based on the phase and duty cycle
        let sample = if self.phase < self.duty_cycle * period {
            self.amplitude as f32
        } else {
            -1.0 * self.amplitude as f32
        };

        // if self.fade_out_samples > 0 {
        //     sample *= self.fade_out_samples as f32 / (FADE_DURATION * self.sample_rate);
        //     self.fade_out_samples -= 1;
        // }

        // if self.fade_in_samples < (FADE_DURATION * self.sample_rate) as u32 {
        //     let factor = self.fade_in_samples as f32 / (FADE_DURATION * self.sample_rate);
        //     sample *= factor * factor;
        //     println!("Sample: {}", sample as u8);
        //     self.fade_in_samples += 1;
        // }

        // Update the phase, wrapping around if necessary
        self.phase = (self.phase + 1.0) % period;

        sample
    }

    fn fade_in(&mut self) {
        self.fade_in_samples = 0;
        self.fade_out_samples = 0;
    }
}

#[derive(Debug, Default)]
pub struct PulseChannel {
    enabled: bool,
    channel_number: usize,
    pub buffer: Arc<Mutex<Vec<f32>>>,
    triggered: bool,
    nrx0: Option<u16>,
    nrx1: u16,
    nrx2: u16,
    nrx3: u16,
    nrx4: u16,
    sample_rate: i32,
    div_apu: u32,
    prev_div_apu_vol: u32,
    prev_div_apu_freq: u32,
    duty_cycle: u8,
    length_timer: u8,
    length_timer_enabled: bool,
    period_value: u16,
    initial_volume: u8,
    volume_envelope_increasing: bool,
    volume_sweep_pace: u8,
    position: usize,
    freq_sweep_period: u8,
    freq_sweep_increase: bool,
    freq_sweep_shift: u8,
    freq_sweep_triggered: bool,
    cycles: u32,
    channel: SquareWaveChannel,
    accumulated_cycles: u32,
}

impl AudioCallback for PulseChannel {
    type Channel = f32;

    fn callback(&mut self, out: &mut [f32]) {
        let mut buffer = self.buffer.lock().unwrap();
        for x in out.iter_mut() {
            if self.position < buffer.len() {
                *x = buffer[self.position];
                self.position += 1;
            } else {
                *x = Self::Channel::SILENCE;
            }
        }

        if self.position > 0 {
            buffer.drain(0..self.position);
            self.position = 0;
        }
    }
}

impl PulseChannel {
    pub fn new(
        channel: usize,
        sample_rate: i32,
        nrx0: Option<u16>,
        nrx1: u16,
        nrx2: u16,
        nrx3: u16,
        nrx4: u16,
    ) -> Self {
        Self {
            enabled: false,
            channel_number: channel,
            buffer: Arc::new(Mutex::new(Vec::new())),
            triggered: false,
            nrx0,
            nrx1,
            nrx2,
            nrx3,
            nrx4,
            sample_rate,
            div_apu: 0,
            prev_div_apu_vol: 0,
            prev_div_apu_freq: 0,
            duty_cycle: 0,
            length_timer: 0,
            length_timer_enabled: false,
            period_value: 0,
            initial_volume: 0,
            volume_envelope_increasing: false,
            volume_sweep_pace: 0,
            position: 0,
            freq_sweep_period: 0,
            freq_sweep_increase: false,
            freq_sweep_shift: 0,
            freq_sweep_triggered: false,
            cycles: 0,
            channel: SquareWaveChannel::new(sample_rate as f32),
            accumulated_cycles: 0,
        }
    }

    fn enable(&mut self, mmu: &mut Mmu) {
        self.enabled = true;
        if self.channel_number == 1 {
            mmu.set(NR52 as u16, mmu.get(NR52) | 0b0000_0001);
        } else if self.channel_number == 2 {
            mmu.set(NR52 as u16, mmu.get(NR52) | 0b0000_0010);
        }
    }

    fn disable(&mut self, mmu: &mut Mmu) {
        self.enabled = false;
        if self.channel_number == 1 {
            mmu.set(NR52 as u16, mmu.get(NR52) & 0b1111_1110);
        } else if self.channel_number == 2 {
            mmu.set(NR52 as u16, mmu.get(NR52) & 0b1111_1101);
        }
    }

    fn update_period(&mut self, mmu: &mut Mmu) {
        let nr10 = self.nrx0.map(|x| mmu.get(x as usize));
        let nr13 = mmu.get(self.nrx3 as usize);
        let nr14 = mmu.get(self.nrx4 as usize);
        let initial_period_value = ((nr14 & 0b0000_0111) as u16) << 8 | nr13 as u16;
        if let Some(nr10) = nr10 {
            self.freq_sweep_increase = (nr10 & 0b0000_1000) == 0;
            self.freq_sweep_shift = nr10 & 0b0000_0111;
            if self.freq_sweep_triggered
                && self.freq_sweep_period != 0
                && (self.div_apu >> 2) - (self.prev_div_apu_freq >> 2)
                    >= self.freq_sweep_period as u32
            {
                self.freq_sweep_triggered = false;
                self.period_value = if self.freq_sweep_increase {
                    let new_period_value =
                        initial_period_value + (initial_period_value >> self.freq_sweep_shift);
                    if new_period_value > 0x7FF {
                        self.disable(mmu);
                        new_period_value
                    } else {
                        new_period_value
                    }
                } else {
                    initial_period_value - (initial_period_value >> self.freq_sweep_shift)
                };
                mmu.set(self.nrx3, (self.period_value & 0xFF) as u8);
                mmu.set(
                    self.nrx4,
                    (nr14 & 0b1100_0000) | (0x7 & (self.period_value >> 8)) as u8,
                );
                self.prev_div_apu_freq = self.div_apu;
            } else {
                self.period_value = initial_period_value;
            }
        } else {
            self.period_value = initial_period_value;
        }
        self.channel.frequency = 131072.0 / (2048.0 - self.period_value as f32);
    }

    fn update_volume(&mut self, mmu: &mut Mmu) {
        let nr12 = mmu.get(self.nrx2 as usize);
        if nr12 & 0b1111_1000 == 0 {
            self.disable(mmu);
            return;
        }
        self.initial_volume = (nr12 & 0b1111_0000) >> 4;
        if self.triggered {
            self.channel.amplitude = self.initial_volume;
            self.volume_envelope_increasing = (nr12 & 0b0000_1000) != 0;
            self.volume_sweep_pace = nr12 & 0b0000_0111;
        } else if self.volume_sweep_pace != 0 {
            if (self.div_apu >> 3) - (self.prev_div_apu_vol >> 3) >= self.volume_sweep_pace as u32 {
                if self.volume_envelope_increasing {
                    self.channel.amplitude = min(15, self.channel.amplitude.saturating_add(1));
                } else {
                    if self.channel_number == 2 {
                        println!("Amplitude: {}", self.channel.amplitude);
                    }
                    self.channel.amplitude = self.channel.amplitude.saturating_sub(1);
                }
                self.prev_div_apu_vol = self.div_apu;
            }
        } else {
            self.channel.amplitude = self.initial_volume;
        }
    }

    fn update_duty_cycle(&mut self, mmu: &Mmu) {
        let nr11 = mmu.get(self.nrx1 as usize);
        self.duty_cycle = (nr11 & 0b1100_0000) >> 6;
        match self.duty_cycle {
            0 => self.channel.duty_cycle = 0.125,
            1 => self.channel.duty_cycle = 0.25,
            2 => self.channel.duty_cycle = 0.5,
            3 => self.channel.duty_cycle = 0.75,
            _ => unreachable!(),
        }
    }

    pub fn generate_sample(&mut self, mmu: &mut Mmu) -> f32 {
        let nr10 = self.nrx0.map(|x| mmu.get(x as usize));
        let nr14 = mmu.get(self.nrx4 as usize);
        if nr14 & 0b1000_0000 != 0 {
            self.triggered = true;
            self.enable(mmu);
            self.freq_sweep_period = nr10.map_or(0, |x| (x & 0b0111_0000) >> 4);
            self.freq_sweep_triggered = true;
        }
        self.update_period(mmu);
        self.update_volume(mmu);
        self.update_duty_cycle(mmu);
        self.triggered = false;
        if self.enabled {
            self.channel.generate_sample()
        } else {
            0.0
        }
    }
}

#[derive(Debug, Default)]
pub struct WaveChannel {
    pub enabled: bool,
    buffer: Arc<Mutex<Vec<f32>>>,
    triggered: bool,
    sample_rate: i32,
    period_value: u16,
    frequency: f32,
    volume: f32,
    phase: f32,
}

impl WaveChannel {
    pub fn new(sample_rate: i32) -> Self {
        Self {
            enabled: false,
            buffer: Arc::new(Mutex::new(Vec::new())),
            triggered: false,
            sample_rate,
            period_value: 0,
            frequency: 0.0,
            volume: 0.0,
            phase: 0.0,
        }
    }

    fn enable(&mut self, mmu: &mut Mmu) {
        self.enabled = true;
        mmu.set(0xFF1A, mmu.get(0xFF1A) | 0b1000_0000);
    }

    fn disable(&mut self, mmu: &mut Mmu) {
        self.enabled = false;
        mmu.set(0xFF1A, mmu.get(0xFF1A) & 0b0111_1111);
    }

    fn generate_sample(&mut self, mmu: &mut Mmu) -> f32 {
        let nr30 = mmu.get(0xFF1A);
        let nr31 = mmu.get(0xFF1B);
        let nr32 = mmu.get(0xFF1C);
        let nr33 = mmu.get(0xFF1D);
        let nr34 = mmu.get(0xFF1E);
        if nr34 & 0b1000_0000 != 0 {
            self.triggered = true;
            self.enable(mmu);
        }
        if nr30 & 0b1000_0000 == 0 {
            self.disable(mmu);
            return 0.0;
        }
        let wave_ram = mmu.get_wave_ram();
        let period_value = ((nr34 & 0b0000_0111) as u16) << 8 | nr33 as u16;
        let frequency = 65536.0 / (2048.0 - period_value as f32);

        let sample_index = (self.phase as usize) % 32;
        let sample = if sample_index % 2 == 0 {
            wave_ram[sample_index / 2] & 0xF
        } else {
            wave_ram[sample_index / 2] >> 4
        };

        let volume_shift: usize = match nr32 & 0b0110_0000 {
            0b0000_0000 => 4,
            0b0010_0000 => 0,
            0b0100_0000 => 1,
            0b0110_0000 => 2,
            _ => unreachable!(),
        };

        let sample = sample >> volume_shift;

        self.phase = (self.phase + frequency / self.sample_rate as f32) % 32.0;

        sample as f32
    }
}
