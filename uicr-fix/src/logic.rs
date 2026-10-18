//! Lógica pura do backup/restauração do UICR (testável no host).

use heapless::Vec;

pub const UICR_WORDS: usize = 194;
pub const PSELRESET_IDX: [usize; 2] = [128, 129];
pub const NRFFW0_IDX: usize = 5;
pub const BOOTLOADER_ADDR: u32 = 0x000E_0000;
pub const PSELRESET_P0_18: u32 = 0x0000_0012;
pub const PSELRESET_UNPINNED: u32 = 0x0000_0010;
pub const BACKUP_MAGIC: u32 = 0x5549_4352; // "UICR"

const ERASED: u32 = 0xFFFF_FFFF;

/// CRC-32/IEEE (refletido, poly 0xEDB88320) sobre os bytes little-endian das palavras.
pub fn crc32(words: &[u32]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for byte in words.iter().flat_map(|w| w.to_le_bytes()) {
        crc ^= byte as u32;
        for _ in 0..8 {
            crc = if crc & 1 != 0 { (crc >> 1) ^ 0xEDB8_8320 } else { crc >> 1 };
        }
    }
    !crc
}

#[derive(Clone, Copy)]
#[repr(C)]
pub struct Backup {
    pub magic: u32,
    pub words: [u32; UICR_WORDS],
    pub crc: u32,
}

impl Backup {
    pub fn new(words: [u32; UICR_WORDS]) -> Backup {
        let mut b = Backup { magic: BACKUP_MAGIC, words, crc: 0 };
        b.crc = b.compute_crc();
        b
    }

    pub fn is_valid(&self) -> bool {
        self.magic == BACKUP_MAGIC && self.crc == self.compute_crc()
    }

    /// CRC de `magic` seguido de `words` (o campo `crc` fica de fora).
    fn compute_crc(&self) -> u32 {
        let mut buf = [0u32; UICR_WORDS + 1];
        buf[0] = self.magic;
        buf[1..].copy_from_slice(&self.words);
        crc32(&buf)
    }
}

/// O backup com os dois PSELRESET apagados.
pub fn expected_after_fix(backup: &[u32; UICR_WORDS]) -> [u32; UICR_WORDS] {
    let mut out = *backup;
    for i in PSELRESET_IDX {
        out[i] = ERASED;
    }
    out
}

#[derive(Debug, PartialEq)]
pub enum RestorePlan {
    Done,
    Write(Vec<(usize, u32), UICR_WORDS>),
    Conflict(usize),
}

pub fn restore_plan(target: &[u32; UICR_WORDS], current: &[u32; UICR_WORDS]) -> RestorePlan {
    if current == target {
        return RestorePlan::Done;
    }
    let mut writes = Vec::new();
    for (i, (&t, &c)) in target.iter().zip(current.iter()).enumerate() {
        if c == t {
            continue;
        }
        if c != ERASED {
            return RestorePlan::Conflict(i);
        }
        // c == ERASED e t != c, logo t != ERASED. Capacidade = UICR_WORDS, nunca estoura.
        let _ = writes.push((i, t));
    }
    RestorePlan::Write(writes)
}

#[derive(Debug, PartialEq)]
pub enum FixCheck {
    Ok,
    NothingToDo,
    Abort(&'static str),
}

pub fn fix_preconditions(current: &[u32; UICR_WORDS], backup_readback: &Backup) -> FixCheck {
    if !backup_readback.is_valid() {
        return FixCheck::Abort("backup inválido");
    }
    if backup_readback.words != *current {
        return FixCheck::Abort("leitura de volta difere");
    }
    if current[NRFFW0_IDX] != BOOTLOADER_ADDR {
        return FixCheck::Abort("NRFFW[0] inesperado");
    }
    if PSELRESET_IDX.iter().all(|&i| current[i] == ERASED) {
        return FixCheck::NothingToDo;
    }
    if PSELRESET_IDX.iter().any(|&i| current[i] != PSELRESET_P0_18) {
        return FixCheck::Abort("PSELRESET inesperado");
    }
    FixCheck::Ok
}

/// Um FIX anterior foi interrompido (ou já concluído): há backup válido do estado com
/// bootloader e o UICR atual difere dele. Nesse caso não se regrava o backup nem se apaga o UICR.
pub fn resume_needed(old: &Backup, current: &[u32; UICR_WORDS]) -> bool {
    old.is_valid() && old.words[NRFFW0_IDX] == BOOTLOADER_ADDR && old.words != *current
}

#[derive(Debug, PartialEq)]
pub enum UnpinCheck {
    Write,
    NothingToDo,
    Abort(&'static str),
}

/// Decide se o UNPIN grava PSELRESET[1] (0x12 -> 0x10, só limpando bits, sem apagar).
pub fn unpin_check(current: &[u32; UICR_WORDS]) -> UnpinCheck {
    if current[NRFFW0_IDX] != BOOTLOADER_ADDR {
        return UnpinCheck::Abort("NRFFW[0] inesperado");
    }
    let (a, b) = (current[PSELRESET_IDX[0]], current[PSELRESET_IDX[1]]);
    if a == PSELRESET_P0_18 && b == PSELRESET_UNPINNED {
        UnpinCheck::NothingToDo
    } else if a == PSELRESET_P0_18 && b == PSELRESET_P0_18 {
        UnpinCheck::Write
    } else {
        UnpinCheck::Abort("PSELRESET inesperado")
    }
}

#[derive(Debug, PartialEq)]
pub enum Command {
    Fix,
    Dfu,
    Wipe,
    Dump,
    Unpin,
    Unknown,
}

pub fn parse_command(line: &[u8]) -> Command {
    let line = line.trim_ascii();
    if line.eq_ignore_ascii_case(b"fix") {
        Command::Fix
    } else if line.eq_ignore_ascii_case(b"dfu") {
        Command::Dfu
    } else if line.eq_ignore_ascii_case(b"wipe") {
        Command::Wipe
    } else if line.eq_ignore_ascii_case(b"dump") {
        Command::Dump
    } else if line.eq_ignore_ascii_case(b"unpin") {
        Command::Unpin
    } else {
        Command::Unknown
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> [u32; UICR_WORDS] {
        let mut w = [0xFFFF_FFFF; UICR_WORDS];
        w[NRFFW0_IDX] = BOOTLOADER_ADDR; w[6] = 0x000F_E000;          // NRFFW[0], NRFFW[1]
        w[PSELRESET_IDX[0]] = PSELRESET_P0_18; w[PSELRESET_IDX[1]] = PSELRESET_P0_18;
        w[0x304 / 4] = 0x0000_0004;                                    // REGOUT0 = 3V0
        w
    }
    #[test] fn expected_after_fix_only_clears_pselreset() {
        let e = expected_after_fix(&sample());
        assert_eq!(e[128], 0xFFFF_FFFF); assert_eq!(e[129], 0xFFFF_FFFF);
        assert_eq!(e[NRFFW0_IDX], BOOTLOADER_ADDR); assert_eq!(e[0x304 / 4], 4); assert_eq!(e[6], 0x000F_E000);
    }
    #[test] fn restore_plan_from_erased_writes_every_programmed_word() {
        let t = expected_after_fix(&sample());
        let RestorePlan::Write(w) = restore_plan(&t, &[0xFFFF_FFFF; UICR_WORDS]) else { panic!() };
        assert_eq!(w.as_slice(), &[(5, BOOTLOADER_ADDR), (6, 0x000F_E000), (0x304 / 4, 4)]);
    }
    #[test] fn restore_plan_done() {
        let t = expected_after_fix(&sample());
        assert!(matches!(restore_plan(&t, &t), RestorePlan::Done));
        let mut cur = [0xFFFF_FFFF; UICR_WORDS]; cur[6] = 0;
        assert!(matches!(restore_plan(&t, &cur), RestorePlan::Conflict(6)));
    }
    #[test] fn backup_roundtrip_and_crc() {
        let mut b = Backup::new(sample()); assert!(b.is_valid());
        b.words[6] ^= 1; assert!(!b.is_valid());
    }
    #[test] fn fix_preconditions_cases() {
        let s = sample();
        assert!(matches!(fix_preconditions(&s, &Backup::new(s)), FixCheck::Ok));
        assert!(matches!(fix_preconditions(&expected_after_fix(&s), &Backup::new(expected_after_fix(&s))), FixCheck::NothingToDo));
        let mut other = s; other[0x304 / 4] = 5;                       // leitura de volta ≠ UICR atual
        assert!(matches!(fix_preconditions(&s, &Backup::new(other)), FixCheck::Abort(_)));
        let mut bad = Backup::new(s); bad.crc ^= 1;
        assert!(matches!(fix_preconditions(&s, &bad), FixCheck::Abort(_)));
        let mut nobl = s; nobl[NRFFW0_IDX] = 0xFFFF_FFFF;
        assert!(matches!(fix_preconditions(&nobl, &Backup::new(nobl)), FixCheck::Abort(_)));
    }
    #[test] fn parse_command_cases() {
        assert!(matches!(parse_command(b"fix\r\n"), Command::Fix));
        assert!(matches!(parse_command(b"  DFU "), Command::Dfu));
        assert!(matches!(parse_command(b"Wipe\n"), Command::Wipe));
        assert!(matches!(parse_command(b"dump\r\n"), Command::Dump));
        assert!(matches!(parse_command(b"unpin\r\n"), Command::Unpin));
        assert!(matches!(parse_command(b"erase"), Command::Unknown));
    }
    #[test] fn resume_needed_cases() {
        let s = sample();
        assert!(!resume_needed(&Backup::new(s), &s));                       // primeira execução: UICR igual
        assert!(resume_needed(&Backup::new(s), &expected_after_fix(&s)));   // já corrigido ou interrompido
        assert!(resume_needed(&Backup::new(s), &[0xFFFF_FFFF; UICR_WORDS]));
        let mut bad = Backup::new(s); bad.crc ^= 1;
        assert!(!resume_needed(&bad, &[0xFFFF_FFFF; UICR_WORDS]));          // backup inválido
        let mut nobl = s; nobl[NRFFW0_IDX] = 0xFFFF_FFFF;
        assert!(!resume_needed(&Backup::new(nobl), &s));                    // backup sem bootloader
    }
    #[test] fn unpin_check_cases() {
        let s = sample();                                   // PSELRESET 0x12/0x12, NRFFW0 ok
        assert!(matches!(unpin_check(&s), UnpinCheck::Write));
        let mut done = s; done[PSELRESET_IDX[1]] = PSELRESET_UNPINNED;
        assert!(matches!(unpin_check(&done), UnpinCheck::NothingToDo));
        let mut erased = s; erased[PSELRESET_IDX[0]] = 0xFFFF_FFFF; erased[PSELRESET_IDX[1]] = 0xFFFF_FFFF;
        assert!(matches!(unpin_check(&erased), UnpinCheck::Abort(_)));
        let mut other = s; other[PSELRESET_IDX[0]] = 0x10;
        assert!(matches!(unpin_check(&other), UnpinCheck::Abort(_)));
        let mut nobl = s; nobl[NRFFW0_IDX] = 0xFFFF_FFFF;
        assert!(matches!(unpin_check(&nobl), UnpinCheck::Abort(_)));
    }
}
