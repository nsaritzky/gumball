use std::fs::File;
use std::io::Read;
use std::ops::{Index, IndexMut};

pub struct Mmu {
    memory: [u8; 0x10000],
    total_rom: Vec<u8>,
}

impl Mmu {
    pub fn new() -> Self {
        Mmu {
            memory: [0u8; 0x10000],
            total_rom: Vec::new(),
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
        self.memory[0x0000..0x4000].copy_from_slice(&rom[0..0x4000]);
        self.total_rom = rom;
    }

    fn switch_rom_bank(&mut self, bank: u8) {
        let bank = bank & 0x1F;
        let bank = if bank == 0 { 1usize } else { bank as usize };
        let offset = bank * 0x4000;
        let temp = self.total_rom[offset..offset + 0x4000].to_vec();
        self.memory[0x4000..0x8000].copy_from_slice(&temp);
    }

    pub fn set(&mut self, address: u16, value: u8) {
        match address {
            0x2000..=0x3FFF => self.switch_rom_bank(value),
            0xFF04 => self.memory[address as usize] = 0,
            address => self.memory[address as usize] = value,
        }
    }

    pub fn get(&self, address: usize) -> u8 {
        self.memory[address]
    }

    pub fn inc_div(&mut self) {
        self.memory[0xFF04] = self.memory[0xFF04].wrapping_add(1);
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
    let mut file = File::open(path)?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)?;
    Ok(buffer)
}
