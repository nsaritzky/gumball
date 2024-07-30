use std::ops::BitAnd;

use crate::cpu::Cpu;
use crate::mmu::Mmu;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Interrupt {
    VBlank,
    LcdStat,
    Timer,
    Serial,
    Joypad,
}

pub fn get_interrupts(mem: &Mmu) -> Vec<Interrupt> {
    let byte = mem.get(0xFF0F);
    let mut interrupts = Vec::new();
    if byte & 0b00001 != 0 {
        interrupts.push(Interrupt::VBlank);
    }
    if byte & 0b00010 != 0 {
        interrupts.push(Interrupt::LcdStat);
    }
    if byte & 0b00100 != 0 {
        interrupts.push(Interrupt::Timer);
    }
    if byte & 0b01000 != 0 {
        interrupts.push(Interrupt::Serial);
    }
    if byte & 0b10000 != 0 {
        interrupts.push(Interrupt::Joypad);
    }
    interrupts
}

impl BitAnd<u8> for Interrupt {
    type Output = bool;

    fn bitand(self, rhs: u8) -> bool {
        match self {
            Interrupt::VBlank => rhs & 0b00001 != 0,
            Interrupt::LcdStat => rhs & 0b00010 != 0,
            Interrupt::Timer => rhs & 0b00100 != 0,
            Interrupt::Serial => rhs & 0b01000 != 0,
            Interrupt::Joypad => rhs & 0b10000 != 0,
        }
    }
}

impl Interrupt {
    fn priority(&self) -> u8 {
        match self {
            Interrupt::VBlank => 0,
            Interrupt::LcdStat => 1,
            Interrupt::Timer => 2,
            Interrupt::Serial => 3,
            Interrupt::Joypad => 4,
        }
    }

    pub fn address(&self) -> u16 {
        match self {
            Interrupt::VBlank => 0x40,
            Interrupt::LcdStat => 0x48,
            Interrupt::Timer => 0x50,
            Interrupt::Serial => 0x58,
            Interrupt::Joypad => 0x60,
        }
    }

    pub fn enabled(&self, mem: &Mmu) -> bool {
        let ie = mem.get(0xFFFF) & (1 << self.priority()) != 0;
        let if_ = mem.get(0xFF0F) & (1 << self.priority()) != 0;
        ie && if_
    }

    pub fn clear(&self, mem: &mut Mmu) {
        let mut if_ = mem.get(0xFF0F);
        if_ &= !(1 << self.priority());
        mem.set(0xFF0F, if_);
    }

    pub fn trigger(&self, mem: &mut Mmu) {
        let mut if_ = mem.get(0xFF0F);
        if_ |= 1 << self.priority();
        mem.set(0xFF0F, if_);
    }
}
