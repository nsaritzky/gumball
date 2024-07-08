use crate::mmu::Mmu;
use crate::registers::LCDC;
use crate::WindowCreator;

pub struct BackgroundDisplay {
    window_creator: WindowCreator,
}

impl BackgroundDisplay {
    pub fn new(window_creator: WindowCreator) -> Self {
        Self { window_creator }
    }

    pub fn get_tiles(&self, mem: &Mmu) -> Vec<u8> {
        let base = if mem.get(LCDC) & 0x8 == 0 {
            0x9800
        } else {
            0x9c00
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

        let mut texture = self
            .window_creator
            .texture_creator
            .create_texture_streaming(sdl2::pixels::PixelFormatEnum::RGB24, 256, 256)
            .map_err(|e| e.to_string())?;

        let pitch = 256 * 3;

        texture
            .with_lock(None, |buffer: &mut [u8], _| {
                for (i, &tile) in tiles.iter().enumerate() {
                    let x = (i % 32) * 8;
                    let y = (i / 32) * 8;
                    for j in 0..8 {
                        let b12 = !((mem.get(LCDC) & 0x10) != 0 || (tile & 0x80) != 0);
                        let tile_addr = 0x8000 | ((b12 as u16) << 12) | ((tile as u16) << 4);
                        let byte1 = mem.get(tile_addr as usize + j * 2);
                        let byte2 = mem.get(tile_addr as usize + j * 2 + 1);
                        for k in 0..8 {
                            let bit1 = (byte1 >> (7 - k)) & 1;
                            let bit2 = (byte2 >> (7 - k)) & 1;
                            let color_index = (bit1 << 1) | bit2;
                            let color = match color_index {
                                0 => [255, 255, 255],
                                1 => [192, 192, 192],
                                2 => [96, 96, 96],
                                3 => [0, 0, 0],
                                _ => unreachable!(),
                            };
                            let offset = (y + j) * pitch + (x + k) * 3;
                            buffer[offset] = color[0];
                            buffer[offset + 1] = color[1];
                            buffer[offset + 2] = color[2];
                        }
                    }
                }
            })
            .map_err(|e| e.to_string())?;

        self.window_creator.canvas.copy(&texture, None, None)?;
        self.window_creator.canvas.present();
        Ok(())
    }
}
