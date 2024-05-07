use std::env;
use std::fs::File;
use std::io::{self, Read};

fn read_binary_file(path: &str) -> io::Result<Vec<u8>> {
    let mut file = File::open(path)?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)?;
    Ok(buffer)
}

fn disassemble_instr(data: &[u8]) -> Result<(String, usize), String> {
    macro_rules! instr {
        ($s:expr) => {
            Ok((format!($s), 0))
        };
        ($s:expr, 1) => {
            Ok((format!($s, data[1]), 1))
        };
        ($s:expr, 2) => {
            Ok((format!($s, u16::from_le_bytes([data[1], data[2]])), 2))
        };
    }

    match data[0] {
        0x0 => instr!("NOP"),
        0x01 => instr!("LD BC, {:#04x}", 1),
        0x02 => instr!("LD (BC),A"),
        0x03 => instr!("INC BC"),
        0x04 => instr!("INC B"),
        0x05 => instr!("DEC B"),
        0x06 => instr!("LD B, {:#04x}", 1),
        0x07 => instr!("RLCA"),
        0x08 => instr!("LD {:#04x}, SP", 2),
        0x09 => instr!("ADD HL, BC"),
        0x0A => instr!("LD A, [BC]"),
        0x0B => instr!("DEC BC"),
        0x0C => instr!("INC C"),
        0x0D => instr!("DEC C"),
        0x0E => instr!("LD C, {:#04x}", 1),
        0x0F => instr!("RRCA"),

        0x10 => instr!("STOP {:#04x}", 1),
        0x11 => instr!("LD DE, {:#04x}", 2),
        0x12 => instr!("LD [DE], A"),
        0x13 => instr!("INC DE"),
        0x14 => instr!("INC D"),
        0x15 => instr!("DEC D"),
        0x16 => instr!("LD D, {:#04x}", 1),
        0x17 => instr!("RLA"),
        0x18 => instr!("JR {:#04x}", 1),
        0x19 => instr!("ADD HL, DE"),
        0x1A => instr!("LD A, [DE]"),
        0x1B => instr!("DEC DE"),
        0x1C => instr!("INC E"),
        0x1D => instr!("DEC E"),
        0x1E => instr!("LD E, {:#04x}", 1),
        0x1F => instr!("RRA"),

        0x20 => instr!("JR NZ, {:#04x}", 1),
        0x21 => instr!("LD HL, {:#04x}", 2),
        0x22 => instr!("LD [HL+], A"),
        0x23 => instr!("INC HL"),
        0x24 => instr!("INC H"),
        0x25 => instr!("DEC H"),
        0x26 => instr!("LD [HL], {:#04x}", 1),
        0x27 => instr!("SCF"),
        0x28 => instr!("JR C, {:#04x}", 1),
        0x29 => instr!("ADD HL, HL"),
        0x2A => instr!("LD A, [HL+]"),
        0x2B => instr!("DEC HL"),
        0x2C => instr!("INC L"),
        0x2D => instr!("DEC L"),
        0x2E => instr!("LD L, {:#04x}", 1),
        0x2F => instr!("CPL"),

        0x30 => instr!("JR NC, {:#04x}", 1),
        0x31 => instr!("LD SP, {:#04x}", 2),
        0x32 => instr!("LD [HL-], A"),
        0x33 => instr!("INC SP"),
        0x34 => instr!("INC [HL]"),
        0x35 => instr!("DEC [HL]"),
        0x36 => instr!("LD [HL], {:#04x}", 1),
        0x37 => instr!("SCF"),
        0x38 => instr!("JR C, {:#04x}", 1),
        0x39 => instr!("ADD HL, SP"),
        0x3A => instr!("LD A, [HL-]"),
        0x3B => instr!("DEC SP"),
        0x3C => instr!("INC A"),
        0x3D => instr!("DEC A"),
        0x3E => instr!("LD A, {:#04x}", 1),
        0x3F => instr!("CCF"),

        0x40 => instr!("LD B, B"),
        0x41 => instr!("LD B, C"),
        0x42 => instr!("LD B, D"),
        0x43 => instr!("LD B, E"),
        0x44 => instr!("LD B, H"),
        0x45 => instr!("LD B, L"),
        0x46 => instr!("LD B, [HL]"),
        0x47 => instr!("LD B, A"),
        0x48 => instr!("LD C, B"),
        0x49 => instr!("LD C, C"),
        0x4A => instr!("LD C, D"),
        0x4B => instr!("LD C, E"),
        0x4C => instr!("LD C, H"),
        0x4D => instr!("LD C, L"),
        0x4E => instr!("LD C, [HL]"),
        0x4F => instr!("LD C, A"),

        0x50 => instr!("LD D, B"),
        0x51 => instr!("LD D, C"),
        0x52 => instr!("LD D, D"),
        0x53 => instr!("LD D, E"),
        0x54 => instr!("LD D, H"),
        0x55 => instr!("LD D, L"),
        0x56 => instr!("LD D, [HL]"),
        0x57 => instr!("LD D, A"),
        0x58 => instr!("LD E, B"),
        0x59 => instr!("LD E, C"),
        0x5A => instr!("LD E, D"),
        0x5B => instr!("LD E, E"),
        0x5C => instr!("LD E, H"),
        0x5D => instr!("LD E, L"),
        0x5E => instr!("LD E, [HL]"),
        0x5F => instr!("LD E, A"),

        0x60 => instr!("LD H, B"),
        0x61 => instr!("LD H, C"),
        0x62 => instr!("LD H, D"),
        0x63 => instr!("LD H, E"),
        0x64 => instr!("LD H, H"),
        0x65 => instr!("LD H, L"),
        0x66 => instr!("LD H, [HL]"),
        0x67 => instr!("LD H, A"),
        0x68 => instr!("LD L, B"),
        0x69 => instr!("LD L, C"),
        0x6A => instr!("LD L, D"),
        0x6B => instr!("LD L, E"),
        0x6C => instr!("LD L, H"),
        0x6D => instr!("LD L, L"),
        0x6E => instr!("LD L, [HL]"),
        0x6F => instr!("LD L, A"),

        0x70 => instr!("LD [HL], B"),
        0x71 => instr!("LD [HL], C"),
        0x72 => instr!("LD [HL], D"),
        0x73 => instr!("LD [HL], E"),
        0x74 => instr!("LD [HL], H"),
        0x75 => instr!("LD [HL], L"),
        0x76 => instr!("HALT"),
        0x77 => instr!("LD [HL], A"),
        0x78 => instr!("LD A, B"),
        0x79 => instr!("LD A, C"),
        0x7A => instr!("LD A, D"),
        0x7B => instr!("LD A, E"),
        0x7C => instr!("LD A, H"),
        0x7D => instr!("LD A, L"),
        0x7E => instr!("LD A, [HL]"),
        0x7F => instr!("LD A, A"),

        0x80 => instr!("ADD A, B"),
        0x81 => instr!("ADD A, C"),
        0x82 => instr!("ADD A, D"),
        0x83 => instr!("ADD A, E"),
        0x84 => instr!("ADD A, H"),
        0x85 => instr!("ADD A, L"),
        0x86 => instr!("ADD A, [HL]"),
        0x87 => instr!("ADD A, A"),
        0x88 => instr!("ADC A, B"),
        0x89 => instr!("ADC A, C"),
        0x8A => instr!("ADC A, D"),
        0x8B => instr!("ADC A, E"),
        0x8C => instr!("ADC A, H"),
        0x8D => instr!("ADC A, L"),
        0x8E => instr!("ADC A, [HL]"),
        0x8F => instr!("ADC A, A"),

        0x90 => instr!("SUB A, B"),
        0x91 => instr!("SUB A, C"),
        0x92 => instr!("SUB A, D"),
        0x93 => instr!("SUB A, E"),
        0x94 => instr!("SUB A, H"),
        0x95 => instr!("SUB A, L"),
        0x96 => instr!("SUB A, [HL]"),
        0x97 => instr!("SUB A, A"),
        0x98 => instr!("SBC A, B"),
        0x99 => instr!("SBC A, C"),
        0x9A => instr!("SBC A, D"),
        0x9B => instr!("SBC A, E"),
        0x9C => instr!("SBC A, H"),
        0x9D => instr!("SBC A, L"),
        0x9E => instr!("SBC A, [HL]"),
        0x9F => instr!("SBC A, A"),

        0xA0 => instr!("AND A, B"),
        0xA1 => instr!("AND A, C"),
        0xA2 => instr!("AND A, D"),
        0xA3 => instr!("AND A, E"),
        0xA4 => instr!("AND A, H"),
        0xA5 => instr!("AND A, L"),
        0xA6 => instr!("AND A, [HL]"),
        0xA7 => instr!("AND A, A"),
        0xA8 => instr!("XOR A, B"),
        0xA9 => instr!("XOR A, C"),
        0xAA => instr!("XOR A, D"),
        0xAB => instr!("XOR A, E"),
        0xAC => instr!("XOR A, H"),
        0xAD => instr!("XOR A, L"),
        0xAE => instr!("XOR A, [HL]"),
        0xAF => instr!("XOR A, A"),

        0xB0 => instr!("OR A, B"),
        0xB1 => instr!("OR A, C"),
        0xB2 => instr!("OR A, D"),
        0xB3 => instr!("OR A, E"),
        0xB4 => instr!("OR A, H"),
        0xB5 => instr!("OR A, L"),
        0xB6 => instr!("OR A, [HL]"),
        0xB7 => instr!("OR A, A"),
        0xB8 => instr!("CP A, B"),
        0xB9 => instr!("CP A, C"),
        0xBA => instr!("CP A, D"),
        0xBB => instr!("CP A, E"),
        0xBC => instr!("CP A, H"),
        0xBD => instr!("CP A, L"),
        0xBE => instr!("CP A, [HL]"),
        0xBF => instr!("CP A, A"),

        0xC0 => instr!("RET NZ"),
        0xC1 => instr!("POP BC"),
        0xC2 => instr!("JP NZ, {:#04x}", 2),
        0xC3 => instr!("JP {:#04x}", 2),
        0xC4 => instr!("CALL NZ, {:#04x}", 2),
        0xC5 => instr!("PUSH BC"),
        0xC6 => instr!("ADD A, {:#04x}", 1),
        0xC7 => instr!("RST $00"),
        0xC8 => instr!("RET Z"),
        0xC9 => instr!("RET"),
        0xCA => instr!("JP Z, {:#04x}", 2),
        0xCB => instr!("PREFIX"),
        0xCC => instr!("CALL Z, {:#04x}", 2),
        0xCD => instr!("CALL {:#04x}", 2),
        0xCE => instr!("ADC A, {:#04x}", 1),
        0xCF => instr!("RST $08"),

        0xD0 => instr!("RET NC"),
        0xD1 => instr!("POP DE"),
        0xD2 => instr!("JP NC, {:#04x}", 2),
        // D3
        0xD4 => instr!("CALL NC, {:#04x}", 2),
        0xD5 => instr!("PUSH DE"),
        0xD6 => instr!("SUB A, {:#04x}", 1),
        0xD7 => instr!("RST $10"),
        0xD8 => instr!("RET C"),
        0xD9 => instr!("RETI"),
        0xDA => instr!("JP C, {:#04x}", 2),
        // DB
        0xDC => instr!("CALL C, {:#04x}", 2),
        // DD
        0xDE => instr!("SBC A, {:#04x}", 1),
        0xDF => instr!("RST $18"),

        0xE0 => instr!("LD [{:#04x}], A", 1),
        0xE1 => instr!("POP HL"),
        0xE2 => instr!("LD [C], A"),
        0xE5 => instr!("PUSH HL"),
        0xE6 => instr!("AND A, {:#04x}", 1),
        0xE7 => instr!("RST $20"),
        0xE8 => instr!("ADD SP, {:#04x}", 1),
        0xE9 => instr!("JP HL"),
        0xEA => instr!("LD [{:#04x}], A", 2),
        0xEE => instr!("XOR A, {:#04x}", 1),
        0xEF => instr!("RST $28"),

        0xF0 => instr!("LD A, [{:#04x}]", 1),
        0xF1 => instr!("POP AF"),
        0xF2 => instr!("LD A, [C]"),
        0xF3 => instr!("DI"),
        0xF5 => instr!("PUSH AF"),
        0xF6 => instr!("OR A, {:#04x}", 1),
        0xF7 => instr!("RST $30"),
        0xF8 => instr!("LD HL, SP + {:#04x}", 1),
        0xF9 => instr!("LD SP, HL"),
        0xFA => instr!("LD A, [{:#04x}]", 2),
        0xFB => instr!("EI"),
        0xFE => instr!("CP A, {:#04x}", 1),
        0xFF => instr!("RST $38"),

        op => Err(format!("Unrecognized opcode {op}")),
    }
}

fn disassemble(data: Vec<u8>) {
    let mut pc = 0;

    while pc < data.len() {
        match disassemble_instr(&data[pc..]) {
            Ok((out_str, inc)) => {
                println!("{}", out_str);
                pc = pc + 1 + inc;
            }
            Err(out_str) => {
                println!("{}", out_str);
                break;
            }
        }
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage: {} <path_to_file>", args[0]);
        std::process::exit(1);
    }

    let file_path = &args[1];

    match read_binary_file(file_path) {
        Ok(data) => disassemble(data),
        Err(e) => println!("Failure :( {}", e),
    }
}
