use std::usize;

use crate::interrupts::{get_interrupts, Interrupt};
use crate::mmu::Mmu;

const CLOCK_SPEED: u64 = 1_050_000;
const DIV_RATE: u64 = 16_384;

fn add_8_8(x: u8, y: u8, val: u16) -> (u8, u8, bool, bool) {
    let a = (u16::from(x) << 8) | u16::from(y);
    let (result, overflow) = a.overflowing_add(val);
    (
        (result >> 8) as u8,
        (result & 0xFF) as u8,
        overflow,                            // c flag
        (a & 0xFFF) + (val & 0xFFF) > 0xFFF, // h flag
    )
}

fn inc_8_8(x: u8, y: u8) -> (u8, u8) {
    let (b, carry) = y.overflowing_add(1);
    let a = if carry { x.wrapping_add(1) } else { x };
    (a, b)
}

fn dec_8_8(x: u8, y: u8) -> (u8, u8) {
    let (b, carry) = y.overflowing_sub(1);
    let a = if carry { x.wrapping_sub(1) } else { x };
    (a, b)
}

fn flag_to_u8(x: bool) -> u8 {
    if x {
        1u8
    } else {
        0u8
    }
}

#[derive(Default, Debug, Clone, Copy)]
struct Flags {
    z: bool,
    n: bool,
    h: bool,
    c: bool,
}

#[derive(Debug, Clone, Copy)]
struct Registers {
    a: u8,
    b: u8,
    c: u8,
    d: u8,
    e: u8,
    h: u8,
    l: u8,
}

impl Default for Registers {
    fn default() -> Self {
        Registers {
            a: 0x01,
            b: 0x00,
            c: 0x13,
            d: 0x00,
            e: 0xD8,
            h: 0x01,
            l: 0x4D,
        }
    }
}

impl Registers {
    fn get_bc(&self) -> u16 {
        let b = self.b as u16;
        let c = self.c as u16;
        (b << 8) | c
    }

    fn set_bc(&mut self, val: u16) {
        self.b = (val >> 8) as u8;
        self.c = (val & 0xFF) as u8;
    }

    fn get_de(&self) -> u16 {
        let d = self.d as u16;
        let e = self.e as u16;
        (d << 8) | e
    }

    fn set_de(&mut self, val: u16) {
        self.d = (val >> 8) as u8;
        self.e = (val & 0xFF) as u8;
    }

    fn get_hl(&self) -> u16 {
        let h = self.h as u16;
        let l = self.l as u16;
        (h << 8) | l
    }

    fn set_hl(&mut self, val: u16) {
        self.h = (val >> 8) as u8;
        self.l = (val & 0xFF) as u8;
    }

    fn inc_hl(&mut self) {
        let (l, carry) = self.l.overflowing_add(1);
        let h = if carry {
            self.h.wrapping_add(1)
        } else {
            self.h
        };
        self.h = h;
        self.l = l;
    }

    fn dec_hl(&mut self) {
        let (l, carry) = self.l.overflowing_sub(1);
        let h = if carry {
            self.h.wrapping_sub(1)
        } else {
            self.h
        };
        self.h = h;
        self.l = l;
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Cpu {
    registers: Registers,
    flags: Flags,
    pc: usize,
    sp: usize,
    ime: bool,
    ime_delay: bool,
    pub halted: bool,
    pub stopped: bool,
    clock_cycles: usize,
}

impl Default for Cpu {
    fn default() -> Self {
        Cpu {
            registers: Default::default(),
            flags: Flags {
                z: true,
                n: false,
                h: true,
                c: true,
            },
            pc: 0x0100,
            sp: 0xFFFE,
            ime: false,
            ime_delay: false,
            halted: false,
            stopped: false,
            clock_cycles: 0,
        }
    }
}

#[derive(PartialEq, Clone, Copy)]
enum R8 {
    B,
    C,
    D,
    E,
    H,
    L,
    HLMem,
    A,
}

fn r8(code: u8) -> R8 {
    match code {
        0 => R8::B,
        1 => R8::C,
        2 => R8::D,
        3 => R8::E,
        4 => R8::H,
        5 => R8::L,
        6 => R8::HLMem,
        7 => R8::A,
        _ => panic!("Invalid R8 code"),
    }
}

enum R16 {
    BC,
    DE,
    HL,
    SP,
}

fn r16(opcode: u8) -> R16 {
    match opcode {
        0x00 => R16::BC,
        0x01 => R16::DE,
        0x02 => R16::HL,
        0x03 => R16::SP,
        op => panic!("Invalid r16 mem code {op}"),
    }
}

enum R16Mem {
    BC,
    DE,
    HLI, // increment
    HLD, // decrement
}

fn r16_mem(x: u8) -> R16Mem {
    match x {
        0 => R16Mem::BC,
        1 => R16Mem::DE,
        2 => R16Mem::HLI,
        3 => R16Mem::HLD,
        x => panic!("Invalid r16 mem code {x}"),
    }
}

enum Cond {
    NZ,
    Z,
    NC,
    C,
}

fn cond(code: u8) -> Cond {
    match code {
        0 => Cond::NZ,
        1 => Cond::Z,
        2 => Cond::NC,
        3 => Cond::C,
        n => panic!("Invalid cond code {n}"),
    }
}

enum R16Stk {
    BC,
    DE,
    HL,
    AF,
}

fn r16stk(code: u8) -> R16Stk {
    match code {
        0 => R16Stk::BC,
        1 => R16Stk::DE,
        2 => R16Stk::HL,
        3 => R16Stk::AF,
        x => panic!("Invalid r16stk code {x}"),
    }
}

fn ld_r16(pair: R16, cpu: &mut Cpu, val: u16) {
    match pair {
        R16::BC => cpu.registers.set_bc(val),
        R16::DE => cpu.registers.set_de(val),
        R16::HL => cpu.registers.set_hl(val),
        R16::SP => {
            cpu.sp = val as usize;
        }
    }
}

fn ld_r16_mem_a(pair: R16Mem, cpu: &mut Cpu, mem: &mut Mmu) {
    match pair {
        R16Mem::BC => {
            let addr = cpu.registers.get_bc();
            mem.set(addr, cpu.registers.a);
        }
        R16Mem::DE => {
            let addr = cpu.registers.get_de();
            mem.set(addr, cpu.registers.a);
        }
        R16Mem::HLD => {
            let addr = cpu.registers.get_hl();
            mem.set(addr, cpu.registers.a);
            cpu.registers.dec_hl();
        }
        R16Mem::HLI => {
            let addr = cpu.registers.get_hl();
            mem.set(addr, cpu.registers.a);
            cpu.registers.inc_hl();
        }
    }
}

fn ld_a_r16_mem(pair: R16Mem, cpu: &mut Cpu, mem: &mut Mmu) {
    match pair {
        R16Mem::BC => {
            let addr = cpu.registers.get_bc();
            cpu.registers.a = mem.get(addr as usize);
        }
        R16Mem::DE => {
            let addr = cpu.registers.get_de();
            cpu.registers.a = mem.get(addr as usize);
        }
        R16Mem::HLD => {
            let addr = cpu.registers.get_hl();
            cpu.registers.a = mem.get(addr as usize);

            cpu.registers.dec_hl();
        }
        R16Mem::HLI => {
            let addr = cpu.registers.get_hl();
            cpu.registers.a = mem.get(addr as usize);

            cpu.registers.inc_hl();
        }
    }
}

fn ld_imm16_sp(cpu: &mut Cpu, mem: &mut Mmu, addr: u16) {
    mem.set(addr, (cpu.sp & 0xFF) as u8);
    mem.set(addr + 1, (cpu.sp >> 8) as u8);
}

fn inc_r16(cpu: &mut Cpu, opcode: u8) {
    match r16((opcode & 0b00110000) >> 4) {
        R16::BC => {
            (cpu.registers.b, cpu.registers.c) = inc_8_8(cpu.registers.b, cpu.registers.c);
        }
        R16::DE => {
            (cpu.registers.d, cpu.registers.e) = inc_8_8(cpu.registers.d, cpu.registers.e);
        }
        R16::HL => {
            (cpu.registers.h, cpu.registers.l) = inc_8_8(cpu.registers.h, cpu.registers.l);
        }
        R16::SP => {
            cpu.sp += 1;
        }
    }
}

fn dec_r16(cpu: &mut Cpu, opcode: u8) {
    match r16((opcode & 0b00110000) >> 4) {
        R16::BC => {
            (cpu.registers.b, cpu.registers.c) = dec_8_8(cpu.registers.b, cpu.registers.c);
        }
        R16::DE => {
            (cpu.registers.d, cpu.registers.e) = dec_8_8(cpu.registers.d, cpu.registers.e);
        }
        R16::HL => {
            (cpu.registers.h, cpu.registers.l) = dec_8_8(cpu.registers.h, cpu.registers.l);
        }
        R16::SP => {
            cpu.sp = cpu.sp.wrapping_sub(1);
        }
    }
}

fn inc_r8(cpu: &mut Cpu, mem: &mut Mmu, opcode: u8) -> u64 {
    let result;
    let mut cycles = 1;
    match r8((opcode & 0b00111000) >> 3) {
        R8::B => {
            result = cpu.registers.b.wrapping_add(1);
            cpu.registers.b = result;
        }
        R8::C => {
            result = cpu.registers.c.wrapping_add(1);
            cpu.registers.c = result;
        }
        R8::D => {
            result = cpu.registers.d.wrapping_add(1);
            cpu.registers.d = result;
        }
        R8::E => {
            result = cpu.registers.e.wrapping_add(1);
            cpu.registers.e = result;
        }
        R8::H => {
            result = cpu.registers.h.wrapping_add(1);
            cpu.registers.h = result;
        }
        R8::L => {
            result = cpu.registers.l.wrapping_add(1);
            cpu.registers.l = result;
        }
        R8::HLMem => {
            result = mem[cpu.registers.get_hl() as usize].wrapping_add(1);
            mem[cpu.registers.get_hl() as usize] = result;
            cycles += 2;
        }
        R8::A => {
            result = cpu.registers.a.wrapping_add(1);
            cpu.registers.a = result;
        }
    }
    cpu.flags.z = result == 0;
    cpu.flags.n = false;
    cpu.flags.h = result & 0xF == 0;
    cycles
}

fn dec_r8(cpu: &mut Cpu, mem: &mut Mmu, opcode: u8) -> u64 {
    let result;
    let mut cycles = 1;
    match r8((opcode & 0b00111000) >> 3) {
        R8::B => {
            result = cpu.registers.b.wrapping_sub(1);
            cpu.registers.b = result;
        }
        R8::C => {
            result = cpu.registers.c.wrapping_sub(1);
            cpu.registers.c = result;
        }
        R8::D => {
            result = cpu.registers.d.wrapping_sub(1);
            cpu.registers.d = result;
        }
        R8::E => {
            result = cpu.registers.e.wrapping_sub(1);
            cpu.registers.e = result;
        }
        R8::H => {
            result = cpu.registers.h.wrapping_sub(1);
            cpu.registers.h = result;
        }
        R8::L => {
            result = cpu.registers.l.wrapping_sub(1);
            cpu.registers.l = result;
        }
        R8::HLMem => {
            result = mem[cpu.registers.get_hl() as usize].wrapping_sub(1);
            mem.set(cpu.registers.get_hl(), result);
            cycles += 2;
        }
        R8::A => {
            result = cpu.registers.a.wrapping_sub(1);
            cpu.registers.a = result;
        }
    }
    cpu.flags.z = result == 0;
    cpu.flags.n = true;
    cpu.flags.h = result & 0xF == 0xF;
    cycles
}

fn ld_r8_imm(state: &mut Cpu, mem: &mut Mmu, opcode: u8, val: u8) -> u64 {
    let mut cycles = 2;
    match r8((opcode & 0b00111000) >> 3) {
        R8::B => {
            state.registers.b = val;
        }
        R8::C => {
            state.registers.c = val;
        }
        R8::D => {
            state.registers.d = val;
        }
        R8::E => {
            state.registers.e = val;
        }
        R8::H => {
            state.registers.h = val;
        }
        R8::L => {
            state.registers.l = val;
        }
        R8::HLMem => {
            cycles += 1;
            mem.set(state.registers.get_hl(), val);
        }
        R8::A => {
            state.registers.a = val;
        }
    }
    cycles
}

fn rotate_left(state: &mut Cpu, through_carry_flag: bool, val: u8) -> u8 {
    let b7 = val >> 7;
    let result = if through_carry_flag {
        (val << 1) | if state.flags.c { 1 } else { 0 }
    } else {
        (val << 1) | b7
    };
    state.flags.c = b7 == 1;
    result
}

fn rotate_right(state: &mut Cpu, through_carry_flag: bool, val: u8) -> u8 {
    let b0 = val & 1u8;
    let result = if through_carry_flag {
        (if state.flags.c { 1 << 7 } else { 0 }) | (val >> 1)
    } else {
        (b0 << 7) | (val >> 1)
    };
    state.flags.c = b0 == 1;
    result
}

fn jr(state: &mut Cpu, mem: &Mmu) {
    let val = mem.get(state.pc + 1) as i8;
    state.pc += 2;
    state.pc = state.pc.wrapping_add_signed(val.into());
}

fn jp(state: &mut Cpu, mem: &Mmu) {
    state.pc = u16::from_le_bytes([mem.get(state.pc + 1), mem.get(state.pc + 2)]).into();
}

fn jr_cond(state: &mut Cpu, mem: &Mmu, opcode: u8) -> u64 {
    match cond((0b00011000 & opcode) >> 3) {
        Cond::NZ => {
            if !state.flags.z {
                jr(state, mem);
                3
            } else {
                state.pc += 2;
                2
            }
        }
        Cond::Z => {
            if state.flags.z {
                jr(state, mem);
                3
            } else {
                state.pc += 2;
                2
            }
        }
        Cond::NC => {
            if !state.flags.c {
                jr(state, mem);
                3
            } else {
                state.pc += 2;
                2
            }
        }
        Cond::C => {
            if state.flags.c {
                jr(state, mem);
                3
            } else {
                state.pc += 2;
                2
            }
        }
    }
}

fn jp_cond(state: &mut Cpu, mem: &Mmu, opcode: u8) -> u64 {
    match cond((0b00011000 & opcode) >> 3) {
        Cond::NZ => {
            if !state.flags.z {
                jp(state, mem);
                4
            } else {
                state.pc += 3;
                3
            }
        }
        Cond::Z => {
            if state.flags.z {
                jp(state, mem);
                4
            } else {
                state.pc += 3;
                3
            }
        }
        Cond::NC => {
            if !state.flags.c {
                jp(state, mem);
                4
            } else {
                state.pc += 3;
                3
            }
        }
        Cond::C => {
            if state.flags.c {
                jp(state, mem);
                4
            } else {
                state.pc += 3;
                3
            }
        }
    }
}

fn get_register_value(state: &mut Cpu, mem: &Mmu, register: R8) -> u8 {
    match register {
        R8::B => state.registers.b,
        R8::C => state.registers.c,
        R8::D => state.registers.d,
        R8::E => state.registers.e,
        R8::H => state.registers.h,
        R8::L => state.registers.l,
        R8::HLMem => {
            state.clock_cycles += 1;
            mem.get(state.registers.get_hl() as usize)
        }
        R8::A => state.registers.a,
    }
}

fn set_register_value(state: &mut Cpu, mem: &mut Mmu, register: R8, value: u8) {
    match register {
        R8::B => state.registers.b = value,
        R8::C => state.registers.c = value,
        R8::D => state.registers.d = value,
        R8::E => state.registers.e = value,
        R8::H => state.registers.h = value,
        R8::L => state.registers.l = value,
        R8::HLMem => {
            mem.set(state.registers.get_hl(), value);
            state.clock_cycles += 1;
        }
        R8::A => state.registers.a = value,
    }
}

fn halt(state: &mut Cpu, _mem: &mut Mmu) -> u64 {
    state.halted = true;
    1
}

fn ld_r8_r8(state: &mut Cpu, mem: &mut Mmu, opcode: u8) -> u64 {
    let dest = r8((opcode & 0b00111000) >> 3);
    let src = r8(opcode & 0b00000111);
    if src == R8::HLMem && dest == R8::HLMem {
        return halt(state, mem);
    }
    let src_value = get_register_value(state, mem, src);
    set_register_value(state, mem, dest, src_value);
    if src == R8::HLMem || dest == R8::HLMem {
        2
    } else {
        1
    }
}

type Binop = fn(&mut Cpu, val: u8) -> Flags;

fn operate(state: &mut Cpu, mem: &mut Mmu, opcode: u8, operator: Binop) -> (Flags, u64) {
    let operand = r8(opcode & 0b00000111);
    let val = get_register_value(state, mem, operand);
    (
        operator(state, val),
        if operand == R8::HLMem { 2 } else { 1 },
    )
}

fn operate_imm(state: &mut Cpu, mem: &Mmu, operator: Binop) -> Flags {
    let val = mem[state.pc + 1];
    operator(state, val)
}

fn add(state: &mut Cpu, val: u8) -> Flags {
    let prev = state.registers.a;
    let carry;
    (state.registers.a, carry) = prev.overflowing_add(val);
    Flags {
        z: state.registers.a == 0,
        n: false,
        h: (prev & 0x0F) + (val & 0x0F) > 0x0F,
        c: carry,
    }
}

fn adc(state: &mut Cpu, val: u8) -> Flags {
    let prev = state.registers.a;
    let carry1;
    let carry2;
    (state.registers.a, carry1) = prev.overflowing_add(val);
    (state.registers.a, carry2) = state.registers.a.overflowing_add(flag_to_u8(state.flags.c));
    Flags {
        z: state.registers.a == 0,
        n: false,
        h: (prev & 0x0F) + (val & 0x0F) + flag_to_u8(state.flags.c) > 0x0F,
        c: carry1 || carry2,
    }
}

fn sub(state: &mut Cpu, val: u8) -> Flags {
    let prev = state.registers.a;
    let carry;
    (state.registers.a, carry) = prev.overflowing_sub(val);
    Flags {
        z: state.registers.a == 0,
        n: true,
        h: (prev & 0x0F) < (val & 0x0F),
        c: carry,
    }
}

fn sbc(state: &mut Cpu, val: u8) -> Flags {
    let prev = state.registers.a;
    let carry1;
    let carry2;
    (state.registers.a, carry1) = prev.overflowing_sub(val);
    (state.registers.a, carry2) = state.registers.a.overflowing_sub(flag_to_u8(state.flags.c));
    Flags {
        z: state.registers.a == 0,
        n: true,
        h: (prev & 0x0F) < (val & 0x0F) + flag_to_u8(state.flags.c),
        c: carry1 || carry2,
    }
}

fn and_(state: &mut Cpu, val: u8) -> Flags {
    state.registers.a &= val;
    Flags {
        z: state.registers.a == 0,
        n: false,
        h: true,
        c: false,
    }
}

fn xor_(state: &mut Cpu, val: u8) -> Flags {
    state.registers.a ^= val;
    Flags {
        z: state.registers.a == 0,
        n: false,
        h: false,
        c: false,
    }
}

fn or_(state: &mut Cpu, val: u8) -> Flags {
    state.registers.a |= val;
    Flags {
        z: state.registers.a == 0,
        n: false,
        h: false,
        c: false,
    }
}

fn cp(state: &mut Cpu, val: u8) -> Flags {
    Flags {
        z: state.registers.a == val,
        n: true,
        h: (state.registers.a & 0x0F) < (val & 0x0F),
        c: state.registers.a < val,
    }
}

fn ret(state: &mut Cpu, mem: &mut Mmu) {
    state.pc = (mem.get(state.sp + 1) as usize) << 8 | mem.get(state.sp) as usize;
    state.sp += 2;
}

fn ret_cond(state: &mut Cpu, mem: &mut Mmu, opcode: u8) -> u64 {
    match cond((opcode & 0b00011000) >> 3) {
        Cond::NZ => {
            if !state.flags.z {
                ret(state, mem);
                5
            } else {
                state.pc += 1;
                2
            }
        }
        Cond::Z => {
            if state.flags.z {
                ret(state, mem);
                5
            } else {
                state.pc += 1;
                2
            }
        }
        Cond::NC => {
            if !state.flags.c {
                ret(state, mem);
                5
            } else {
                state.pc += 1;
                2
            }
        }
        Cond::C => {
            if state.flags.c {
                ret(state, mem);
                5
            } else {
                state.pc += 1;
                2
            }
        }
    }
}

fn call(state: &mut Cpu, mem: &mut Mmu) {
    state.sp -= 1;
    mem.set(state.sp as u16, ((state.pc + 3) >> 8) as u8);
    state.sp -= 1;
    mem.set(state.sp as u16, ((state.pc + 3) & 0xFF) as u8);

    jp(state, mem);
    state.clock_cycles += 6;
}

fn call_cond(state: &mut Cpu, mem: &mut Mmu, opcode: u8) -> u64 {
    match cond((0b00011000 & opcode) >> 3) {
        Cond::NZ => {
            if !state.flags.z {
                call(state, mem);
                6
            } else {
                state.pc += 3;
                3
            }
        }
        Cond::Z => {
            if state.flags.z {
                call(state, mem);
                6
            } else {
                state.pc += 3;
                3
            }
        }
        Cond::NC => {
            if !state.flags.c {
                call(state, mem);
                6
            } else {
                state.pc += 3;
                3
            }
        }
        Cond::C => {
            if state.flags.c {
                call(state, mem);
                6
            } else {
                state.pc += 3;
                3
            }
        }
    }
}

fn pop_r16stk(state: &mut Cpu, mem: &mut Mmu, opcode: u8) {
    match r16stk((opcode & 0b00110000) >> 4) {
        R16Stk::AF => {
            let f = mem.get(state.sp);
            state.flags = Flags {
                z: (f & 0b10000000) >> 7 == 1,
                n: (f & 0b01000000) >> 6 == 1,
                h: (f & 0b00100000) >> 5 == 1,
                c: (f & 0b00010000) >> 4 == 1,
            };
            state.sp += 1;
            state.registers.a = mem.get(state.sp);
            state.sp += 1;
        }
        R16Stk::BC => {
            state.registers.c = mem.get(state.sp);
            state.sp += 1;
            state.registers.b = mem.get(state.sp);
            state.sp += 1;
        }
        R16Stk::DE => {
            state.registers.e = mem.get(state.sp);
            state.sp += 1;
            state.registers.d = mem.get(state.sp);
            state.sp += 1;
        }
        R16Stk::HL => {
            state.registers.l = mem.get(state.sp);
            state.sp += 1;
            state.registers.h = mem.get(state.sp);
            state.sp += 1;
        }
    }
}

fn push_r16stk(state: &mut Cpu, mem: &mut Mmu, opcode: u8) {
    match r16stk((opcode & 0b00110000) >> 4) {
        R16Stk::AF => {
            state.sp -= 1;
            mem.set(state.sp as u16, state.registers.a);
            state.sp -= 1;
            mem.set(
                state.sp as u16,
                flag_to_u8(state.flags.z) << 7
                    | flag_to_u8(state.flags.n) << 6
                    | flag_to_u8(state.flags.h) << 5
                    | flag_to_u8(state.flags.c) << 4,
            );
        }
        R16Stk::BC => {
            state.sp -= 1;
            mem.set(state.sp as u16, state.registers.b);
            state.sp -= 1;
            mem.set(state.sp as u16, state.registers.c);
        }
        R16Stk::DE => {
            state.sp -= 1;
            mem.set(state.sp as u16, state.registers.d);
            state.sp -= 1;
            mem.set(state.sp as u16, state.registers.e);
        }
        R16Stk::HL => {
            state.sp -= 1;
            mem.set(state.sp as u16, state.registers.h);
            state.sp -= 1;
            mem.set(state.sp as u16, state.registers.l);
        }
    }
}

impl Cpu {
    pub fn new() -> Self {
        Cpu {
            registers: Registers::default(),
            flags: Flags::default(),
            pc: 0x0100,
            sp: 0xFFFE,
            ime: false,
            ime_delay: false,
            halted: false,
            stopped: false,
            clock_cycles: 0,
        }
    }

    fn get_f_register(&self) -> u8 {
        let b7 = if self.flags.z { 1 } else { 0 };
        let b6 = if self.flags.n { 1 } else { 0 };
        let b5 = if self.flags.h { 1 } else { 0 };
        let b4 = if self.flags.c { 1 } else { 0 };
        b7 << 7 | b6 << 6 | b5 << 5 | b4 << 4
    }

    pub fn enable_ime_delayed(&mut self) {
        if self.ime_delay {
            self.ime = true;
            self.ime_delay = false;
        }
    }

    pub fn execute(&mut self, mem: &mut Mmu) -> u64 {
        let opcode = mem.get(self.pc);
        let clock_cycles;
        match opcode {
            // NOP
            0x00 => {
                clock_cycles = 1;
                self.pc += 1;
            }
            // ld r16, imm16
            op if 0b11001111 & op == 0b00000001 => {
                let register_pair = r16((op & 0b00110000) >> 4);
                let imm16 = u16::from_le_bytes([mem.get(self.pc + 1), mem.get(self.pc + 2)]);
                ld_r16(register_pair, self, imm16);

                clock_cycles = 3;
                self.pc += 3;
            }
            // ld [r16mem], a
            op if 0b11001111 & op == 0b00000010 => {
                ld_r16_mem_a(r16_mem((op & 0b00110000) >> 4), self, mem);

                clock_cycles = 2;
                self.pc += 1
            }
            // ld a, [r16mem]
            op if 0b11001111 & op == 0b00001010 => {
                ld_a_r16_mem(r16_mem((op & 0b00110000) >> 4), self, mem);

                clock_cycles = 2;
                self.pc += 1;
            }
            // ld [imm16], sp
            0x08 => {
                ld_imm16_sp(
                    self,
                    mem,
                    u16::from_le_bytes([mem.get(self.pc + 1), mem.get(self.pc + 2)]),
                );

                clock_cycles = 5;
                self.pc += 3;
            }
            // inc r16
            op if 0b11001111 & op == 0b00000011 => {
                inc_r16(self, op);

                clock_cycles = 2;
                self.pc += 1;
            }
            // dec r16
            op if 0b11001111 & op == 0b00001011 => {
                dec_r16(self, op);

                clock_cycles = 2;
                self.pc += 1;
            }
            // add hl, r16
            op if 0b11001111 & op == 0b00001001 => {
                let operand = r16((op & 0b00110000) >> 4);
                match operand {
                    R16::BC => {
                        let (h, l, cflag, hflag) =
                            add_8_8(self.registers.b, self.registers.c, self.registers.get_hl());
                        self.registers.set_hl(u16::from(h) << 8 | u16::from(l));
                        self.flags.n = false;
                        self.flags.c = cflag;
                        self.flags.h = hflag;
                    }
                    R16::DE => {
                        let (h, l, cflag, hflag) =
                            add_8_8(self.registers.d, self.registers.e, self.registers.get_hl());
                        self.registers.set_hl(u16::from(h) << 8 | u16::from(l));
                        self.flags.n = false;
                        self.flags.c = cflag;
                        self.flags.h = hflag;
                    }
                    R16::HL => {
                        let (h, l, cflag, hflag) =
                            add_8_8(self.registers.h, self.registers.l, self.registers.get_hl());
                        self.registers.set_hl(u16::from(h) << 8 | u16::from(l));
                        self.flags.n = false;
                        self.flags.c = cflag;
                        self.flags.h = hflag;
                    }
                    R16::SP => {
                        let (h, l, cflag, hflag) = add_8_8(
                            (self.sp >> 8) as u8,
                            (self.sp & 0xFF) as u8,
                            self.registers.get_hl(),
                        );
                        self.registers.set_hl(u16::from(h) << 8 | u16::from(l));
                        self.flags.n = false;
                        self.flags.c = cflag;
                        self.flags.h = hflag;
                    }
                }

                clock_cycles = 2;
                self.pc += 1;
            }
            // INC r8
            op if 0b11000111 & op == 0b00000100 => {
                clock_cycles = inc_r8(self, mem, op);
                self.pc += 1;
            }
            // DEC r8
            op if 0b11000111 & op == 0b00000101 => {
                clock_cycles = dec_r8(self, mem, op);
                self.pc += 1;
            }
            // LD r8, imm8
            op if 0b11000111 & op == 0b00000110 => {
                clock_cycles = ld_r8_imm(self, mem, op, mem.get(self.pc + 1));
                self.pc += 2;
            }
            // RLCA
            0x07 => {
                self.registers.a = rotate_left(self, false, self.registers.a);
                self.flags.z = false;
                self.flags.n = false;
                self.flags.h = false;

                clock_cycles = 1;
                self.pc += 1;
            }
            // RRCA
            0x0F => {
                self.registers.a = rotate_right(self, false, self.registers.a);
                self.flags.z = false;
                self.flags.n = false;
                self.flags.h = false;

                clock_cycles = 1;
                self.pc += 1;
            }
            // RLA
            0x17 => {
                self.registers.a = rotate_left(self, true, self.registers.a);
                self.flags.z = false;
                self.flags.n = false;
                self.flags.h = false;

                clock_cycles = 1;
                self.pc += 1;
            }
            // RRA
            0x1F => {
                self.registers.a = rotate_right(self, true, self.registers.a);
                self.flags.z = false;
                self.flags.n = false;
                self.flags.h = false;

                clock_cycles = 1;
                self.pc += 1;
            }
            // DAA
            // Code adapted from https://forums.nesdev.org/viewtopic.php?p=196282&sid=b1d399755b0f63e5d709a5d21bf1492e#p196282
            0x27 => {
                if !self.flags.n {
                    if self.flags.c || self.registers.a > 0x99 {
                        self.registers.a = self.registers.a.wrapping_add(0x60);
                        self.flags.c = true;
                    }
                    if self.flags.h || (self.registers.a & 0x0F) > 0x09 {
                        self.registers.a = self.registers.a.wrapping_add(0x6);
                    }
                } else {
                    if self.flags.c {
                        self.registers.a = self.registers.a.wrapping_sub(0x60);
                    }
                    if self.flags.h {
                        self.registers.a = self.registers.a.wrapping_sub(0x6);
                    }
                }
                self.flags.z = self.registers.a == 0;
                self.flags.h = false;

                clock_cycles = 1;
                self.pc += 1;
            }
            // CPL
            0x2F => {
                self.registers.a = !self.registers.a;
                self.flags.n = true;
                self.flags.h = true;

                clock_cycles = 1;
                self.pc += 1;
            }
            // SCF
            0x37 => {
                self.flags.n = false;
                self.flags.h = false;
                self.flags.c = true;

                clock_cycles = 1;
                self.pc += 1;
            }
            // CCF
            0x3F => {
                self.flags.n = false;
                self.flags.h = false;
                self.flags.c = !self.flags.c;

                clock_cycles = 1;
                self.pc += 1;
            }
            // JR imm8
            0x18 => {
                let val = mem.get(self.pc + 1) as i8;
                self.pc += 2;
                self.pc = self.pc.wrapping_add_signed(val.into());

                clock_cycles = 3;
            }
            // JR COND, imm8
            op if 0b11100111 & op == 0b00100000 => {
                clock_cycles = jr_cond(self, mem, op);
            }
            // STOP
            0x10 => {
                mem.set(0xFF04, 0); // reset DIV register
                self.pc += 2;
                clock_cycles = 1;
            }
            // LD r8, r8
            op if 0b11000000 & op == 0b01000000 => {
                clock_cycles = ld_r8_r8(self, mem, op);
                self.pc += 1;
            }
            // ADD A, r8
            op if 0b11111000 & op == 0b10000000 => {
                (self.flags, clock_cycles) = operate(self, mem, op, add);
                self.pc += 1;
            }
            // ADC A, r8
            op if 0b11111000 & op == 0b10001000 => {
                (self.flags, clock_cycles) = operate(self, mem, op, adc);
                self.pc += 1;
            }
            // SUB A, r8
            op if 0b11111000 & op == 0b10010000 => {
                (self.flags, clock_cycles) = operate(self, mem, op, sub);
                self.pc += 1;
            }
            // SBC A, r8
            op if 0b11111000 & op == 0b10011000 => {
                (self.flags, clock_cycles) = operate(self, mem, op, sbc);
                self.pc += 1;
            }
            // AND A, r8
            op if 0b11111000 & op == 0b10100000 => {
                (self.flags, clock_cycles) = operate(self, mem, op, and_);
                self.pc += 1;
            }
            // XOR A, r8
            op if 0b11111000 & op == 0b10101000 => {
                (self.flags, clock_cycles) = operate(self, mem, op, xor_);
                self.pc += 1;
            }
            // OR A, r8
            op if 0b11111000 & op == 0b10110000 => {
                (self.flags, clock_cycles) = operate(self, mem, op, or_);
                self.pc += 1;
            }
            // CP A, r8
            op if 0b11111000 & op == 0b10111000 => {
                (self.flags, clock_cycles) = operate(self, mem, op, cp);
                self.pc += 1;
            }
            // ADD A, imm8
            0xC6 => {
                self.flags = operate_imm(self, mem, add);

                clock_cycles = 2;
                self.pc += 2;
            }
            // ADC A, imm8
            0xCE => {
                self.flags = operate_imm(self, mem, adc);

                clock_cycles = 2;
                self.pc += 2;
            }
            // SUB A, imm8
            0xD6 => {
                self.flags = operate_imm(self, mem, sub);

                clock_cycles = 2;
                self.pc += 2;
            }
            // SBC A, imm8
            0xDE => {
                self.flags = operate_imm(self, mem, sbc);

                clock_cycles = 2;
                self.pc += 2;
            }
            // AND A, imm8
            0xE6 => {
                self.flags = operate_imm(self, mem, and_);

                clock_cycles = 2;
                self.pc += 2;
            }
            // XOR A, imm8
            0xEE => {
                self.flags = operate_imm(self, mem, xor_);

                clock_cycles = 2;
                self.pc += 2;
            }
            // OR A, imm8
            0xF6 => {
                self.flags = operate_imm(self, mem, or_);

                clock_cycles = 2;
                self.pc += 2;
            }
            // CP A, imm8
            0xFE => {
                self.flags = operate_imm(self, mem, cp);

                clock_cycles = 2;
                self.pc += 2;
            }
            // RET COND
            op if 0b11100111 & op == 0b11000000 => {
                clock_cycles = ret_cond(self, mem, op);
            }
            // RET
            0xC9 => {
                ret(self, mem);

                clock_cycles = 4;
            }
            // RETI
            0xD9 => {
                self.ime = true;
                ret(self, mem);

                clock_cycles = 4;
            }
            // JP COND, imm16
            op if 0b11100111 & op == 0b11000010 => {
                clock_cycles = jp_cond(self, mem, op);
            }
            // JP imm16
            0xC3 => {
                jp(self, mem);
                clock_cycles = 4;
            }
            // JP HL
            0xE9 => {
                self.pc = self.registers.get_hl().into();
                clock_cycles = 1;
            }
            // CALL COND, imm16
            op if 0b11100111 & op == 0b11000100 => {
                clock_cycles = call_cond(self, mem, op);
            }
            // CALL, imm16
            0xCD => {
                call(self, mem);
                clock_cycles = 6;
            }
            // RST tgt3
            op if 0b11000111 & op == 0b11000111 => {
                self.sp -= 1;
                mem.set(self.sp as u16, (((self.pc + 1) & 0xFF00) >> 8) as u8);
                self.sp -= 1;
                mem.set(self.sp as u16, ((self.pc + 1) & 0xFF) as u8);

                clock_cycles = 4;
                self.pc = (0b00111000 & op) as usize;
            }
            // POP r16stk
            op if 0b11001111 & op == 0b11000001 => {
                pop_r16stk(self, mem, op);

                clock_cycles = 3;
                self.pc += 1;
            }
            // PUSH R16stk
            op if 0b11001111 & op == 0b11000101 => {
                push_r16stk(self, mem, op);

                clock_cycles = 4;
                self.pc += 1;
            }
            // LDH [C], A
            0xE2 => {
                mem.set(0xFF00 + self.registers.c as u16, self.registers.a);

                clock_cycles = 2;
                self.pc += 1;
            }
            // LDH [imm8], A
            0xE0 => {
                let addr = mem.get(self.pc + 1) as u16;
                mem.set(0xFF00 + addr, self.registers.a);

                clock_cycles = 3;
                self.pc += 2;
            }
            // LD [imm16], A
            0xEA => {
                let addr = (mem.get(self.pc + 2) as u16) << 8 | mem.get(self.pc + 1) as u16;
                mem.set(addr, self.registers.a);

                clock_cycles = 4;
                self.pc += 3;
            }
            // LDH A, [C]
            0xF2 => {
                self.registers.a = mem.get(0xFF00 + self.registers.c as usize);

                clock_cycles = 2;
                self.pc += 1;
            }
            // LDH A, [imm8]
            0xF0 => {
                self.registers.a = mem.get(0xFF00 + mem[self.pc + 1] as usize);

                clock_cycles = 3;
                self.pc += 2;
            }
            // LD A, [imm16]
            0xFA => {
                self.registers.a =
                    mem.get((mem.get(self.pc + 2) as usize) << 8 | mem.get(self.pc + 1) as usize);

                clock_cycles = 4;
                self.pc += 3;
            }
            // ADD SP, imm8
            0xE8 => {
                let diff = mem[self.pc + 1] as i8;
                let prev = self.sp as u16;
                let result = prev.wrapping_add_signed(diff.into());
                self.sp = result as usize;
                self.flags.z = false;
                self.flags.n = false;
                self.flags.h = if diff >= 0 {
                    (prev & 0xF) + ((diff as u16) & 0xF) > 0xF
                } else {
                    ((prev as usize) & 0x0F) >= ((-diff) as usize & 0x0F)
                    // ((prev & 0x0F).wrapping_sub(diff as u16 & 0x0F)) & 0x10 != 0
                };
                self.flags.c = if diff >= 0 {
                    (prev & 0xFF) + ((diff as u16) & 0xFF) > 0xFF
                } else {
                    ((prev as usize) & 0xFF) >= (((-diff) as usize) & 0xFF)
                };
                // if diff == -1 {
                //     println!(
                //         "a: {:#06x}, a - 1: {:#06x}, c: {}, h: {}",
                //         prev,
                //         result,
                //         if self.flags.c { 1 } else { 0 },
                //         if self.flags.h { 1 } else { 0 }
                //     );
                // }
                clock_cycles = 4;
                self.pc += 2;
            }
            // LD HL, SP + imm8
            0xF8 => {
                let diff = mem[self.pc + 1] as i8;
                let prev = self.sp;
                let result = prev.wrapping_add_signed(diff.into());
                self.registers.set_hl(result as u16);
                self.flags.z = false;
                self.flags.n = false;
                self.flags.h = if diff >= 0 {
                    (prev & 0xF) + ((diff as usize) & 0xF) > 0xF
                } else {
                    ((prev as usize) & 0x0F) >= ((-diff) as usize & 0x0F)
                    // ((prev & 0x0F).wrapping_sub(diff as usize & 0x0F)) & 0x10 != 0
                };
                self.flags.c = if diff >= 0 {
                    (prev & 0xFF) + ((diff as usize) & 0xFF) > 0xFF
                } else {
                    ((prev as usize) & 0xFF) >= (((-diff) as usize) & 0xFF)
                };

                clock_cycles = 3;
                self.pc += 2;
            }
            // LD SP, HL
            0xF9 => {
                self.sp = self.registers.get_hl() as usize;

                clock_cycles = 2;
                self.pc += 1;
            }
            // DI
            0xF3 => {
                self.ime = false;

                clock_cycles = 1;
                self.pc += 1;
            }
            // EI
            0xFB => {
                self.ime_delay = true;

                clock_cycles = 1;
                self.pc += 1;
            }
            0xCB => {
                clock_cycles = execute_prefix_cb(self, mem);

                self.pc += 1;
            }
            op => {
                panic!("Unrecognized opcode {:#02x}", op);
            }
        }
        clock_cycles * 4
    }

    fn handle_interrupt(&mut self, mem: &mut Mmu, interrupt: &Interrupt) {
        interrupt.clear(mem);
        self.ime = false;
        self.sp -= 2;
        mem.set(self.sp as u16, (self.pc & 0xFF) as u8);
        mem.set(self.sp as u16 + 1, (self.pc >> 8) as u8);
        self.pc = interrupt.address() as usize;
    }

    pub fn handle_interrupts(&mut self, mem: &mut Mmu) {
        for interrupt in get_interrupts(mem) {
            self.halted = false;
            if self.ime && interrupt.enabled(mem) {
                self.handle_interrupt(mem, &interrupt);
            }
        }
    }

    pub fn log_state(&self, mem: &Mmu) {
        println!("A: {:02X} F: {:02X} B: {:02X} C: {:02X} D: {:02X} E: {:02X} H: {:02X} L: {:02X} SP: {:04X} PC: 00:{:04X} ({:02X} {:02X} {:02X} {:02X})", self.registers.a, self.get_f_register(), self.registers.b, self.registers.c, self.registers.d, self.registers.e, self.registers.h, self.registers.l, self.sp, self.pc, mem[self.pc], mem[self.pc + 1], mem[self.pc + 2], mem[self.pc + 3]);
    }
}

fn sla_r8(state: &mut Cpu, val: u8) -> u8 {
    let new_val = val << 1;
    state.flags.z = new_val == 0;
    state.flags.n = false;
    state.flags.h = false;
    state.flags.c = val & 0b10000000 == 0b10000000;
    new_val
}

fn sra_r8(state: &mut Cpu, val: u8) -> u8 {
    let new_val = (val & 0b10000000) | (val >> 1);
    state.flags.z = new_val == 0;
    state.flags.n = false;
    state.flags.h = false;
    state.flags.c = val & 0b00000001 == 0b00000001;
    new_val
}

fn swap_r8(state: &mut Cpu, val: u8) -> u8 {
    let new_val = (val & 0x0F) << 4 | (val & 0xF0) >> 4;
    state.flags.z = new_val == 0;
    state.flags.n = false;
    state.flags.h = false;
    state.flags.c = false;
    new_val
}

fn srl_r8(state: &mut Cpu, val: u8) -> u8 {
    let new_val = val >> 1;
    state.flags.z = new_val == 0;
    state.flags.n = false;
    state.flags.h = false;
    state.flags.c = val & 0b00000001 == 0b00000001;
    new_val
}

fn bit(state: &mut Cpu, mem: &mut Mmu, opcode: u8) {
    let bit = (opcode & 0b00111000) >> 3;
    let operand = r8(opcode & 0b00000111);
    let val = get_register_value(state, mem, operand);
    state.flags.z = (val & (1 << bit)) == 0;
}

fn res(state: &mut Cpu, mem: &mut Mmu, opcode: u8) {
    let bit = (opcode & 0b00111000) >> 3;
    let operand = r8(opcode & 0b00000111);
    let val = get_register_value(state, mem, operand);
    set_register_value(state, mem, operand, val & !(1 << bit));
}

fn set(state: &mut Cpu, mem: &mut Mmu, opcode: u8) {
    let bit = (opcode & 0b00111000) >> 3;
    let operand = r8(opcode & 0b00000111);
    let val = get_register_value(state, mem, operand);
    set_register_value(state, mem, operand, val | (1 << bit));
}

fn execute_prefix_cb(state: &mut Cpu, mem: &mut Mmu) -> u64 {
    let opcode = mem.get(state.pc + 1);
    let operand = r8(opcode & 0b00000111);
    let val = get_register_value(state, mem, operand);
    match opcode {
        // RLC r8
        op if 0b11111000 & op == 0b00000000 => {
            let new_val = rotate_left(state, false, val);
            set_register_value(state, mem, operand, new_val);

            state.flags.n = false;
            state.flags.h = false;
            state.flags.z = new_val == 0;

            state.pc += 1;
        }
        // RRC r8
        op if 0b11111000 & op == 0b00001000 => {
            let new_val = rotate_right(state, false, val);
            set_register_value(state, mem, operand, new_val);

            state.flags.n = false;
            state.flags.h = false;
            state.flags.z = new_val == 0;

            state.pc += 1;
        }
        // RL r8
        op if 0b11111000 & op == 0b00010000 => {
            let new_val = rotate_left(state, true, val);
            set_register_value(state, mem, operand, new_val);

            state.flags.n = false;
            state.flags.h = false;
            state.flags.z = new_val == 0;

            state.pc += 1;
        }
        // RR r8
        op if 0b11111000 & op == 0b00011000 => {
            let new_val = rotate_right(state, true, val);
            set_register_value(state, mem, operand, new_val);

            state.flags.n = false;
            state.flags.h = false;
            state.flags.z = new_val == 0;

            state.pc += 1;
        }
        // SLA r8
        op if 0b11111000 & op == 0b00100000 => {
            let new_val = sla_r8(state, val);
            set_register_value(state, mem, operand, new_val);

            state.pc += 1;
        }
        // SRA r8
        op if 0b11111000 & op == 0b00101000 => {
            let new_val = sra_r8(state, val);
            set_register_value(state, mem, operand, new_val);

            state.pc += 1;
        }
        // SWAP r8
        op if 0b11111000 & op == 0b00110000 => {
            let new_val = swap_r8(state, val);
            set_register_value(state, mem, operand, new_val);

            state.pc += 1;
        }
        // SRL r8
        op if 0b11111000 & op == 0b00111000 => {
            let new_val = srl_r8(state, val);
            set_register_value(state, mem, operand, new_val);

            state.pc += 1;
        }
        // BIT b, r8
        op if 0b11000000 & op == 0b01000000 => {
            bit(state, mem, op);

            state.flags.n = false;
            state.flags.h = true;
            state.pc += 1;
        }
        // RES b, r8
        op if 0b11000000 & op == 0b10000000 => {
            res(state, mem, op);

            state.pc += 1;
        }
        // SET b, r8
        op if 0b11000000 & op == 0b11000000 => {
            set(state, mem, op);

            state.pc += 1;
        }
        _ => {
            panic!("Unrecognized opcode {:#02x}", opcode);
        }
    }
    // Return 3 cycles for BIT instructions accessing [HL], 4 cycles for other
    // instructions accessing [HL], and 2 cycles for all other instructions
    if operand == R8::HLMem && 0b11000000 & opcode == 0b01000000 {
        3
    } else if operand == R8::HLMem {
        4
    } else {
        2
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_inc_8_8() {
        assert_eq!((0x01, 0x00), inc_8_8(0x00, 0xFF));
    }

    #[test]
    fn test_jr_nz_e8() {
        let mut mem = Mmu::init_with_vec(vec![0x20, 0x02, 0x00, 0x00, 0x01]);
        let mut state: Cpu = Default::default();

        state.flags.z = false;
        execute(&mut state, &mut mem);
        assert_eq!(state.pc, 0x104);

        let mut state2: Cpu = Default::default();
        execute(&mut state2, &mut mem);
        assert_eq!(state2.pc, 0x102);
    }

    #[test]
    fn ld_d_b() {
        let mut mem = Mmu::init_with_vec(vec![0x50, 0x00]);
        let mut state: Cpu = Default::default();

        state.registers.b = 0xAB;
        execute(&mut state, &mut mem);
        assert_eq!(state.registers.d, 0xAB);
    }

    #[test]
    fn ld_hlmem_b() {
        let mut mem = Mmu::init_with_vec(vec![0x70, 0x00, 0x00, 0x00]);
        let mut state: Cpu = Default::default();

        state.registers.set_hl(0x0003);
        state.registers.b = 0xAB;
        execute(&mut state, &mut mem);
        assert_eq!(mem[3], 0xAB);
    }

    #[test]
    fn test_add_a_b() {
        let mut mem = Mmu::init_with_vec(vec![0x80, 0x00]);
        let mut state: Cpu = Default::default();

        state.registers.a = 5;
        state.registers.b = 4;
        execute(&mut state, &mut mem);
        assert_eq!(state.registers.a, 9);
        assert!(!state.flags.z);
        assert!(!state.flags.n);
        assert!(!state.flags.c);

        let mut state2: Cpu = Default::default();
        state2.registers.a = 0xFF;
        state2.registers.b = 1;
        execute(&mut state2, &mut mem);
        assert!(state2.flags.z);
        assert!(state2.flags.c);
    }

    #[test]
    fn test_sub_a_b() {
        let mut mem = Mmu::init_with_vec(vec![0x90, 0x00]);
        let mut state: Cpu = Default::default();

        state.registers.a = 5;
        state.registers.b = 4;
        execute(&mut state, &mut mem);
        assert_eq!(state.registers.a, 1);
        assert!(state.flags.n);
    }

    #[test]
    fn test_and_a_b() {
        let mut mem = Mmu::init_with_vec(vec![0xA0, 0x00]);
        let mut state: Cpu = Default::default();

        state.registers.a = 0b00111100;
        state.registers.b = 0b00001100;
        execute(&mut state, &mut mem);
        assert_eq!(state.registers.a, 0b00001100);
    }

    #[test]
    fn test_or_a_b() {
        let mut mem = Mmu::init_with_vec(vec![0xB0, 0x00]);
        let mut state: Cpu = Default::default();

        state.registers.a = 0b00111100;
        state.registers.b = 0b00001100;
        execute(&mut state, &mut mem);
        assert_eq!(state.registers.a, 0b00111100);
    }

    #[test]
    fn test_ret_nz() {
        let mut mem = Mmu::init_with_vec(vec![0xC0, 0x00, 0x03]);
        let mut state: Cpu = Default::default();

        state.sp = 0x0102;
        state.flags.z = false;
        execute(&mut state, &mut mem);
        assert_eq!(state.pc, 0x3);

        let mut state2: Cpu = Default::default();
        state2.sp = 0x0102;
        execute(&mut state2, &mut mem);
        assert_eq!(state2.pc, 0x101);
    }

    #[test]
    fn test_ldh_a8mem_a() {
        let mut mem = Mmu::init_with_vec(vec![0xE0, 0x0A]);
        let mut state: Cpu = Default::default();

        state.registers.a = 0xAB;
        execute(&mut state, &mut mem);
        assert_eq!(mem[0xFF0A], 0xAB);
    }

    #[test]
    fn test_ld_bc_n16() {
        let mut mem = Mmu::init_with_vec(vec![0x01, 0x12, 0x34]);
        let mut state: Cpu = Default::default();

        execute(&mut state, &mut mem);
        assert_eq!(state.registers.b, 0x34);
        assert_eq!(state.registers.c, 0x12);
    }

    #[test]
    fn test_pop_bc() {
        let mut mem = Mmu::init_with_vec(vec![0xC1, 0x00, 0x00, 0x0A]);
        let mut state: Cpu = Default::default();

        state.sp = 0x0102;
        execute(&mut state, &mut mem);
        println!("{:?}", state);
        assert_eq!(state.registers.get_bc(), 0x0A00);
    }

    #[test]
    fn test_ld_bcmem_a() {
        let mut mem = Mmu::init_with_vec(vec![0x02, 0x00, 0x00, 0x00]);
        let mut state: Cpu = Default::default();
        state.registers.set_bc(0x0003);
        state.registers.a = 0xAB;

        execute(&mut state, &mut mem);
        assert_eq!(mem[3], 0xAB);
    }

    #[test]
    fn test_hli_a() {
        let mut mem = Mmu::init_with_vec(vec![0x22, 0x00, 0x00, 0x00]);
        let mut state: Cpu = Default::default();
        state.registers.set_hl(0x0003);
        state.registers.a = 0xAB;

        execute(&mut state, &mut mem);
        assert_eq!(mem[3], 0xAB);
        assert_eq!(state.registers.get_hl(), 0x0004);
    }

    #[test]
    fn test_jp_nz() {
        let mut mem = Mmu::init_with_vec(vec![0xC2, 0x04, 0x00, 0x00, 0xAB]);
        let mut state: Cpu = Default::default();

        state.flags.z = false;
        execute(&mut state, &mut mem);
        println!("{:?}", state);
        assert_eq!(state.pc, 4);
    }

    #[test]
    fn test_ld_cmem_a() {
        let mut mem = Mmu::init_with_vec(vec![0xE2]);
        let mut state: Cpu = Default::default();

        state.registers.a = 0xAB;
        state.registers.c = 0xAB;
        execute(&mut state, &mut mem);
        assert_eq!(mem[0xFFAB], 0xAB);
    }

    #[test]
    fn inc_bc() {
        let mut mem = Mmu::init_with_vec(vec![0x03, 0x00]);
        let mut state: Cpu = Default::default();

        state.registers.set_bc(0x1000);
        execute(&mut state, &mut mem);

        assert_eq!(state.registers.get_bc(), 0x1001);

        let mut state2: Cpu = Default::default();

        state2.registers.set_bc(0xFFFF);
        execute(&mut state2, &mut mem);
        assert_eq!(state2.registers.get_bc(), 0);
    }

    #[test]
    fn test_jp_imm16() {
        let mut mem = Mmu::init_with_vec(vec![0xC3, 0x34, 0x12]);
        let mut state: Cpu = Default::default();

        execute(&mut state, &mut mem);
        assert_eq!(state.pc, 0x1234);
    }

    #[test]
    fn test_inc_b() {
        let mut mem = Mmu::init_with_vec(vec![0x04, 0x00]);
        let mut state: Cpu = Default::default();

        state.registers.b = 0x0F;
        execute(&mut state, &mut mem);
        assert_eq!(state.registers.b, 0x10);
        assert!(state.flags.h);

        let mut state2: Cpu = Default::default();
        state2.registers.b = 0xFF;
        execute(&mut state2, &mut mem);
        assert_eq!(state2.registers.b, 0);
        assert!(state2.flags.z);
        assert!(!state2.flags.n);
    }
}
