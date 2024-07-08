use sdl2::{render::Canvas, video::Window};

use crate::mmu::Mmu;
use crate::registers::*;

pub struct WindowDisplay {
    canvas: Canvas<Window>,
}

impl WindowDisplay {
    pub fn new(canvas: Canvas<Window>) -> Self {
        Self { canvas }
    }

    pub fn get_tiles(&self, mem: &Mmu) -> Vec<u8> {
        let base = if mem.get(LCDC) & 0x40 == 0 {
            0x9800
        } else {
            0x9C00
        };
        let mut tiles = Vec::new();
        for i in 0..1024 {
            let tile = mem.get(base + i);
            tiles.push(tile);
        }
        tiles
    }

    pub fn draw_tiles(&mut self, mem: &Mmu) -> Result<(), String> {
        let tiles = self.get_tiles(mem);
        for (i, &tile) in tiles.iter().enumerate() {
            let x = (i % 32) * 8;
            let y = (i / 32) * 8;
            for j in 0..8 {
                let b12 = !((mem.get(LCDC) & 0x10) != 0 || (tile & 0x80) != 0);
                let tile_addr = 0x8000 | ((b12 as u16) << 12) | ((tile as u16) << 4);
                let byte1 = mem.get((tile_addr + j * 2) as usize);
                let byte2 = mem.get((tile_addr + j * 2 + 1) as usize);
                for k in 0..8 {
                    let bit1 = (byte1 >> (7 - k)) & 1;
                    let bit2 = (byte2 >> (7 - k)) & 1;
                    let color = (bit1 << 1) | bit2;
                    let color = match color {
                        0 => [255, 255, 255],
                        1 => [192, 192, 192],
                        2 => [96, 96, 96],
                        3 => [0, 0, 0],
                        _ => unreachable!(),
                    };
                    self.canvas
                        .set_draw_color(sdl2::pixels::Color::RGB(color[0], color[1], color[2]));
                    self.canvas.fill_rect(sdl2::rect::Rect::new(
                        x as i32 + k as i32,
                        y as i32 + j as i32,
                        2,
                        1,
                    ))?;
                }
            }
        }
        self.canvas.present();
        Ok(())
    }
}
