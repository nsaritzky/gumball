use std::fs::File;
use std::io::Read;
use std::ops::{Index, IndexMut};

use crate::input::{Button, Input};
use crate::registers::*;

#[derive(Debug, PartialEq, Clone, Copy)]
enum MBC {
    None,
    MBC1,
    MBC2,
    MBC3,
    MBC5,
}

pub struct Mmu {
    memory: [u8; 0x10000],
    total_rom: Vec<u8>,
    total_ram: Vec<u8>,
    ram_bank: usize,
    mbc: MBC,
    pub input: Input,
    has_external_ram: bool,
    enable_external_ram: bool,
}

impl Mmu {
    pub fn new() -> Self {
        Mmu {
            memory: [0u8; 0x10000],
            total_rom: Vec::new(),
            total_ram: Vec::new(),
            ram_bank: 0,
            mbc: MBC::None,
            input: Input::default(),
            has_external_ram: false,
            enable_external_ram: false,
        }
    }

    pub fn init() -> Self {
        let mut mmu = Mmu::new();
        mmu.memory[0xFF00] = 0xCF;
        mmu.memory[0xFF01] = 0x00;
        mmu.memory[0xFF02] = 0x7E;
        mmu.memory[0xFF04] = 0xAB;
        mmu.memory[0xFF05] = 0x00;
        mmu.memory[0xFF06] = 0x00;
        mmu.memory[0xFF07] = 0x00;
        mmu.memory[0xFF10] = 0x80;
        mmu.memory[0xFF11] = 0xBF;
        mmu.memory[0xFF12] = 0xF3;
        mmu.memory[0xFF13] = 0xFF;
        mmu.memory[0xFF14] = 0xBF;
        mmu.memory[0xFF16] = 0x3F;
        mmu.memory[0xFF17] = 0x00;
        mmu.memory[0xFF18] = 0xFF;
        mmu.memory[0xFF19] = 0xBF;
        mmu.memory[0xFF1A] = 0x7F;
        mmu.memory[0xFF1B] = 0xFF;
        mmu.memory[0xFF1C] = 0x9F;
        mmu.memory[0xFF1D] = 0xFF;
        mmu.memory[0xFF1E] = 0xBF;
        mmu.memory[0xFF20] = 0xFF;
        mmu.memory[0xFF21] = 0x00;
        mmu.memory[0xFF22] = 0x00;
        mmu.memory[0xFF23] = 0xBF;
        mmu.memory[0xFF24] = 0x77;
        mmu.memory[0xFF25] = 0xF3;
        mmu.memory[0xFF26] = 0xF1;
        mmu.memory[0xFF40] = 0x91;
        mmu.memory[0xFF41] = 0x85;
        mmu.memory[0xFF42] = 0x00;
        mmu.memory[0xFF43] = 0x00;
        mmu.memory[0xFF44] = 0x90;
        mmu.memory[0xFF45] = 0x00;
        mmu.memory[0xFF46] = 0xFF;
        mmu.memory[0xFF47] = 0xFC;
        mmu.memory[0xFF48] = 0xFF;
        mmu.memory[0xFF49] = 0xFF;
        mmu.memory[0xFF4A] = 0x00;
        mmu.memory[0xFF4B] = 0x00;
        mmu.memory[0xFFFF] = 0x00;
        mmu
    }

    pub fn init_with_vec(rom: Vec<u8>) -> Self {
        let mut mmu = Mmu::init();
        let len = rom.len();
        if len > 0x8000 - 0x100 {
            panic!("ROM too big");
        }
        mmu.memory[0x0100..0x100 + len].copy_from_slice(&rom);
        mmu
    }

    fn load_rom(&mut self, path: &str) -> std::io::Result<Vec<u8>> {
        let mut file = File::open(path)?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)?;
        Ok(buffer)
    }

    pub fn initialize_memory(&mut self, rom: Vec<u8>) {
        self.memory[0x0000..0x8000].copy_from_slice(&rom[0..0x8000]);
        self.total_rom = rom;
        self.total_ram = vec![0u8; 0x8000];
        match self.total_rom[0x147] {
            0x00 => self.mbc = MBC::None,
            0x01..=0x03 => self.mbc = MBC::MBC1,
            0x05..=0x06 => self.mbc = MBC::MBC2,
            0x0F..=0x13 => self.mbc = MBC::MBC3,
            0x19..=0x1E => self.mbc = MBC::MBC5,
            _ => panic!("Unsupported MBC"),
        }
    }

    fn switch_rom_bank(&mut self, bank: u8) {
        if self.mbc == MBC::None {
            return;
        }
        let bank = bank & 0x1F;
        let bank = if bank == 0 { 1usize } else { bank as usize };
        let offset = bank * 0x4000;
        let temp = self.total_rom[offset..offset + 0x4000].to_vec();
        self.memory[0x4000..0x8000].copy_from_slice(&temp);
    }

    fn switch_ram_bank(&mut self, bank: u8) {
        if self.mbc == MBC::None {
            return;
        }
        let bank = bank & 0x03;
        let bank = if bank == 0 { 1usize } else { bank as usize };
        let offset = bank * 0x2000;
        let old_offset = self.ram_bank * 0x2000;
        self.total_ram[old_offset..old_offset + 0x2000]
            .copy_from_slice(&self.memory[0xA000..0xC000]);
        let temp = self.total_ram[offset..offset + 0x2000].to_vec();
        self.memory[0xA000..0xC000].copy_from_slice(&temp);
        self.ram_bank = bank;
    }

    pub fn set(&mut self, address: u16, value: u8) {
        // match self.mbc {
        //     MBC::MBC1 => match address {
        //         0x0000..=0x1FFF => {
        //             self.enable_external_ram = value == 0x0A;
        //         }
        //         0x2000..=0x3FFF => self.switch_rom_bank(value),
        //         0x4000..=0x5FFF => self.switch_ram_bank(value),
        //         0xA000..=0xBFFF => {
        //             if self.enable_external_ram {
        //                 self.memory[address as usize] = value;
        //             }
        //         }
        //         0xFF00 => self.input.write_ff00(value),
        //         0xFF04 => self.memory[address as usize] = 0,
        //         0xFF46 => self.dma_transfer(value),
        //         address => self.memory[address as usize] = value,
        //     },
        // }
        match address {
            0x0000..=0x1FFF => {
                self.enable_external_ram = value == 0x0A;
            }
            0x2000..=0x3FFF => self.switch_rom_bank(value),
            0x4000..=0x5FFF => self.switch_ram_bank(value),
            0xA000..=0xBFFF => {
                if self.enable_external_ram {
                    self.memory[address as usize] = value;
                }
            }
            0xFF00 => self.input.write_ff00(value),
            0xFF04 => self.memory[address as usize] = 0,
            0xFF46 => self.dma_transfer(value),
            address => self.memory[address as usize] = value,
        }
    }

    pub fn get(&self, address: usize) -> u8 {
        match address {
            0xA000..=0xBFFF => {
                if self.enable_external_ram {
                    self.memory[address]
                } else {
                    0xFF
                }
            }
            0xFF00 => self.input.read_ff00(),
            _ => self.memory[address],
        }
    }

    pub fn get_wave_ram(&self) -> &[u8] {
        &self.memory[WAVE_RAM_START..WAVE_RAM_START + 0x10]
    }

    pub fn inc_div(&mut self) {
        self.memory[0xFF04] = self.memory[0xFF04].wrapping_add(1);
    }

    fn dma_transfer(&mut self, address: u8) {
        let address = address as usize * 0x100;
        for i in 0..0xA0 {
            self.memory[0xFE00 + i] = self.memory[address + i];
        }
    }
}

impl Index<usize> for Mmu {
    type Output = u8;

    fn index(&self, index: usize) -> &u8 {
        &self.memory[index]
    }
}

impl IndexMut<usize> for Mmu {
    fn index_mut(&mut self, index: usize) -> &mut u8 {
        if index == 0xFF04 {
            self.memory[index] = 0;
        }
        &mut self.memory[index]
    }
}

pub fn load_rom(path: &str) -> std::io::Result<Vec<u8>> {
    println!("Loading ROM: {}", path);
    let mut file = File::open(path)?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)?;
    Ok(buffer)
}
