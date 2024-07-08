use sdl2::pixels::Color;
use sdl2::pixels::PixelFormatEnum::RGB24;
use sdl2::render::{Canvas, Texture};
use sdl2::video::Window;
use std::cmp::max;
use std::collections::VecDeque;
use std::time::Instant;

use crate::interrupts::Interrupt;
use crate::mmu::Mmu;
use crate::{registers::*, WindowCreator};

// Clock speed in Hz
const CLOCK_SPEED: u32 = 4_194_304;
// Duration of a frame in microseconds
const FRAME_DURATION: u32 = 16_743;
const PIXEL_BUFFER_SIZE: usize = 176 * 176 * 3;

const PALETTE: [Color; 4] = [
    Color::RGB(0x8c, 0xb5, 0x28),
    Color::RGB(0x6c, 0x94, 0x21),
    Color::RGB(0x42, 0x6b, 0x29),
    Color::RGB(0x21, 0x42, 0x31),
];

// Get the value of a bit in a number
fn get_bit<T>(value: T, bit: u32) -> T
where
    T: Into<u16> + From<u8>,
{
    let bits: u16 = value.into();
    let result = bits & (0b1 << bit) != 0;

    if result {
        T::from(1u8)
    } else {
        T::from(0u8)
    }
}

#[derive(Debug, Clone, Copy)]
enum Palette {
    OBP0,
    OBP1,
    BGP,
}

#[derive(Debug, Clone, Copy)]
struct Pixel {
    color: u8,
    palette: Palette,
    priority: bool,
}

#[derive(Debug, Clone, Copy)]
struct OAM {
    y: u8,
    x: u8,
    tile: u8,
    flags: u8,
}

fn read_oam(mem: &Mmu, address: usize) -> OAM {
    OAM {
        y: mem.get(address),
        x: mem.get(address + 1),
        tile: mem.get(address + 2),
        flags: mem.get(address + 3),
    }
}

enum PPUMode {
    HBlank,
    VBlank,
    OAMSearch,
    PixelTransfer,
}

pub struct PPU<'a> {
    bg_fifo: VecDeque<Pixel>,
    sprite_fifo: VecDeque<Pixel>,
    sprite_buffer: Vec<OAM>,
    mode: PPUMode,
    clock_cycles: u32,
    start_time: Instant,
    pixel_buffer: [u8; PIXEL_BUFFER_SIZE],
    lx: u8,
    window_counter: u8,
    tall_sprites: bool,
    canvas: &'a mut Canvas<Window>,
    texture: Texture<'a>,
    cycle_counter: i32,
    mode3_extra_cycles: i32,
}

impl<'a> PPU<'a> {
    pub fn new(canvas: &'a mut Canvas<Window>, texture: Texture<'a>) -> Result<Self, String> {
        Ok(PPU {
            bg_fifo: VecDeque::new(),
            sprite_fifo: VecDeque::new(),
            sprite_buffer: Vec::new(),
            mode: PPUMode::OAMSearch,
            clock_cycles: 0,
            start_time: Instant::now(),
            pixel_buffer: [0; PIXEL_BUFFER_SIZE],
            lx: 0,
            window_counter: 0,
            tall_sprites: false,
            canvas,
            texture,
            cycle_counter: 0,
            mode3_extra_cycles: 0,
        })
    }

    // Return true if a frame has been rendered
    pub fn render(&mut self, mem: &mut Mmu, cycles: i32) -> Result<bool, String> {
        self.cycle_counter += cycles;
        PPU::stat_interrupt(mem);
        match self.mode {
            PPUMode::VBlank => {
                if self.cycle_counter >= 456 {
                    self.cycle_counter -= 456;
                    self.clock_cycles -= 456;
                    mem.set(LY as u16, 0);
                    Interrupt::VBlank.trigger(mem);
                    self.window_counter = 0;
                    self.texture
                        .update(None, &self.pixel_buffer, 160 * 3)
                        .map_err(|e| e.to_string())?;
                    self.canvas.copy(&self.texture, None, None)?;
                    self.canvas.present();
                    self.mode = PPUMode::OAMSearch;
                    return Ok(true);
                }
            }
            PPUMode::OAMSearch => {
                if self.cycle_counter >= 80 {
                    self.tall_sprites = get_bit(mem.get(LCDC), 2) != 0;
                    self.cycle_counter -= 80;
                    self.scan_sprites(mem);
                    self.mode = PPUMode::PixelTransfer;
                }
            }
            PPUMode::PixelTransfer => {
                if self.cycle_counter >= 172 + self.mode3_extra_cycles {
                    self.mode3_extra_cycles = 0;
                    self.draw_line(mem)?;
                    self.cycle_counter -= 172 + self.mode3_extra_cycles;
                    self.mode = PPUMode::HBlank;
                }
            }
            PPUMode::HBlank => {
                if self.cycle_counter >= 289 - self.mode3_extra_cycles {
                    self.cycle_counter -= 289 - self.mode3_extra_cycles;
                    self.lx = 0;
                    if mem.get(WY) <= mem.get(LY) && get_bit(mem.get(LCDC), 5) != 0 {
                        self.window_counter += 1;
                    }
                    if mem.get(LY) == 176 {
                        mem.set(LY as u16, 0);
                        self.mode = PPUMode::VBlank;
                    } else {
                        mem.set(LY as u16, mem.get(LY) + 1);
                        self.mode = PPUMode::OAMSearch;
                    }
                }
            }
        }
        Ok(false)
    }

    fn wait(&mut self, cycles: u32) {
        self.clock_cycles += cycles;
    }

    fn stat_interrupt(mem: &mut Mmu) {
        let stat = mem.get(STAT);
        let mode = stat & 0b11;
        let mode2 = (stat >> 3) & 0b1;
        let mode1 = (stat >> 4) & 0b1;
        let mode0 = (stat >> 5) & 0b1;
        let ly = mem.get(LY);
        let lyc = mem.get(LYC);
        let coincidence = ly == lyc;
        mem.set(
            STAT as u16,
            (stat & 0b11111011) | if coincidence { 0b100 } else { 0 },
        );
        let coincidence_interrupt = (stat >> 6) & 0b1;
        if coincidence && coincidence_interrupt != 0 {
            Interrupt::LcdStat.trigger(mem);
        }
        if mode == 0 && mode0 != 0 {
            Interrupt::LcdStat.trigger(mem);
        } else if mode == 1 && mode1 != 0 {
            Interrupt::LcdStat.trigger(mem);
        } else if mode == 2 && mode2 != 0 {
            Interrupt::LcdStat.trigger(mem);
        }
    }

    fn scan_sprites(&mut self, mem: &Mmu) {
        let mut result = Vec::new();
        for i in 0..40 {
            let sprite_height = if self.tall_sprites { 16 } else { 8 };
            let mut oam = read_oam(mem, 0xFE00 + i * 4);
            if oam.x > 0
                && mem.get(LY) + 16 > oam.y
                && mem.get(LY) + 16 <= oam.y + sprite_height
                && result.len() < 10
            {
                if self.tall_sprites {
                    if mem.get(LY) + 16 < oam.y + 8 {
                        oam.tile = oam.tile & 0xFE;
                    } else {
                        oam.tile = oam.tile | 0x1;
                    }
                }
                result.push(oam);
            }
        }
        self.sprite_buffer = result;
        self.clock_cycles += 80;
    }

    fn fetch_byte(&mut self, mem: &Mmu, addr: u16) -> u8 {
        self.clock_cycles += 2;
        mem.get(addr as usize)
    }

    fn fetch_bg(&mut self, mem: &Mmu) {
        let tile_id_addr = 0x9800
            | (get_bit(mem.get(LCDC), 3) as u16) << 10
            | (mem.get(LY).wrapping_add(mem.get(SCY)) as u16 >> 3) << 5
            | (self.lx.wrapping_add(mem.get(SCX))) as u16 >> 3;
        let tile_id = mem.get(tile_id_addr as usize);
        let b12 = u16::from(!((mem.get(LCDC) & 0x10) != 0 || (tile_id & 0x80) != 0));
        let addr = 0x8000
            | b12 << 12
            | (tile_id as u16) << 4
            | ((mem.get(LY).wrapping_add(mem.get(SCY)) & 0b111) as u16) << 1;
        let low = self.fetch_byte(mem, addr);
        let high = self.fetch_byte(mem, addr + 1);
        self.push_bg_tile_row(low, high);
    }

    fn push_bg_tile_row(&mut self, low: u8, high: u8) {
        self.clock_cycles += 1;
        for i in 0..8 {
            let color = ((low >> (7 - i) & 0b1) << 1) | (high >> (7 - i) & 0b1);
            self.bg_fifo.push_back(Pixel {
                color,
                palette: Palette::BGP,
                priority: false,
            });
        }
    }

    fn fetch_window(&mut self, mem: &Mmu) {
        let tile_id_addr = 0x9800
            | (get_bit(mem.get(LCDC), 6) as u16) << 10
            | (self.window_counter as u16 >> 3) << 5
            | self.lx as u16 >> 3;
        let tile_id = mem.get(tile_id_addr as usize);
        let b12 = u16::from(!((mem.get(LCDC) & 0x10) != 0 || (tile_id & 0x80) != 0));
        let addr: u16 = 0x8000
            | b12 << 12
            | (tile_id as u16) << 4
            | ((mem.get(LY).wrapping_add(mem.get(WY)) & 0b111) as u16) << 1;
        let low = self.fetch_byte(mem, addr);
        let high = self.fetch_byte(mem, addr + 1);
        self.push_bg_tile_row(low, high);
    }

    // Tries to fetch a sprite from the sprite buffer, returns true if it finds one
    fn fetch_obj(&mut self, mem: &Mmu) -> bool {
        let mut candidate_index: Option<usize> = None;
        for (index, sprite) in self.sprite_buffer.iter().enumerate() {
            if let Some(candidate) = candidate_index {
                // In sprites with equal x, the one with the lowest index has priority
                if sprite.x < self.sprite_buffer[candidate].x
                    && self.lx >= sprite.x
                    && self.lx < sprite.x + 8
                {
                    candidate_index = Some(index);
                }
            } else {
                if self.lx >= sprite.x && self.lx < sprite.x + 8 {
                    candidate_index = Some(index);
                }
            }
        }
        if let Some(index) = candidate_index {
            let sprite = self.sprite_buffer[index];
            self.push_sprite_tile_row(mem, &sprite);
            true
        } else {
            false
        }
    }

    fn push_sprite_tile_row(&mut self, mem: &Mmu, sprite: &OAM) {
        let hflip = get_bit(sprite.flags, 5) != 0;
        let vflip = get_bit(sprite.flags, 6) != 0;
        let y = mem.get(LY).wrapping_sub(sprite.y + 1) & 0x7;
        let y = if vflip { 7 - y } else { y };
        let addr = 0x8000 | (sprite.tile as u16) << 4 | (y as u16) << 1;
        let low = mem.get(addr as usize);
        let high = mem.get(addr as usize + 1);
        for i in (self.lx - sprite.x)..8 {
            let x = if hflip { 7 - i } else { i };
            self.sprite_fifo.push_back(Pixel {
                color: ((low >> (7 - x)) & 0b1) | (((high >> (7 - x)) & 0b1) << 1),
                palette: if get_bit(sprite.flags, 4) == 0 {
                    Palette::OBP0
                } else {
                    Palette::OBP1
                },
                priority: get_bit(sprite.flags, 7) != 0,
            });
        }
    }

    fn merge_pixels(&self, mem: &Mmu, bg: Pixel, sprite: Option<Pixel>) -> Pixel {
        if let Some(sprite) = sprite {
            if get_bit(mem.get(LCDC), 0) == 0 {
                sprite
            } else if get_bit(mem.get(LCDC), 1) == 0 {
                bg
            } else if (sprite.priority && bg.color != 0) || sprite.color == 0 {
                bg
            } else {
                sprite
            }
        } else {
            bg
        }
    }

    fn draw_pixel(&mut self, mem: &Mmu, tile_offset: u32) -> Result<i32, String> {
        let window_active =
            get_bit(mem.get(LCDC), 5) != 0 && mem.get(WX) <= self.lx && mem.get(LY) >= mem.get(WY);
        let mut clock_cycles: i32 = 0;
        // If we just reached the window, delay the fetch by 6 cycles
        if window_active && self.lx == mem.get(WX) {
            clock_cycles += 6;
        }

        if self.bg_fifo.len() >= 8 {
            if self.sprite_fifo.is_empty() {
                if self.fetch_obj(mem) {
                    // Following the OBJ penalty algorithm from the pandocs
                    if self.lx == 0 {
                        clock_cycles += 11;
                    } else {
                        clock_cycles += 6 + max(5 - self.lx as i32 - tile_offset as i32, 0);
                    }
                }
            }
            if self.sprite_fifo.is_empty() {
                let pixel = self.bg_fifo.pop_front().unwrap();
                self.render_pixel(mem, pixel)?;
            } else {
                let bg_pixel = self.bg_fifo.pop_front().unwrap();
                let sprite_pixel = self.sprite_fifo.pop_front();
                let pixel = self.merge_pixels(mem, bg_pixel, sprite_pixel);
                self.render_pixel(mem, pixel)?;
            }
            self.lx += 1;
        } else if window_active {
            self.fetch_window(mem);
            clock_cycles += 6;
        } else {
            self.fetch_bg(mem);
            clock_cycles += 6;
        }
        Ok(clock_cycles)
    }

    pub fn draw_line(&mut self, mem: &Mmu) -> Result<i32, String> {
        self.bg_fifo.clear();
        self.sprite_fifo.clear();
        self.lx = 0;

        let tile_offset = mem.get(SCX) as u32 % 8;
        let mut clock_cycles = 0;
        self.clock_cycles += tile_offset;

        while self.lx < 176 {
            clock_cycles += self.draw_pixel(mem, tile_offset)?;
        }
        self.mode3_extra_cycles = clock_cycles;
        Ok(clock_cycles)
    }

    fn render_pixel(&mut self, mem: &Mmu, pixel: Pixel) -> Result<(), String> {
        if self.lx >= 8 && mem.get(LY) < 144 {
            let palette = match pixel.palette {
                Palette::BGP => mem.get(BGP),
                Palette::OBP0 => mem.get(OBP0),
                Palette::OBP1 => mem.get(OBP1),
            };
            let color = PALETTE[(palette >> (pixel.color * 2)) as usize & 0b11];
            let offset = (mem.get(LY) as usize * 160 + self.lx as usize - 8) * 3;
            self.pixel_buffer[offset] = color.r;
            self.pixel_buffer[offset + 1] = color.g;
            self.pixel_buffer[offset + 2] = color.b;
        }
        Ok(())
    }
}
