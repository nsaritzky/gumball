mod cpu;
mod mmu;

use clap::Parser;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    rom_path: String,
}

fn main() {
    let args = Args::parse();
    let rom = mmu::load_rom(&args.rom_path);
    let mut mem = mmu::Mmu::init();

    match rom {
        Ok(rom) => {
            mem.initialize_memory(rom);
            cpu::emulate(&mut mem);
        }
        Err(e) => panic!("Error loading rom: {e}"),
    }
}
