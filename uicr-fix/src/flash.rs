//! Acesso cru ao NVMC (flash e UICR). Cada operação espera `READY` e a CONFIG volta a `Ren`.

use nrf52840_hal::pac;
use uicr_fix::logic::{Backup, UICR_WORDS};

pub const UICR_BASE: u32 = 0x1000_1000;
pub const BACKUP_ADDR: u32 = 0x000D_F000;
const BACKUP_WORDS: usize = UICR_WORDS + 2;
const PAGE_SIZE: u32 = 4096;

fn nvmc() -> &'static pac::nvmc::RegisterBlock {
    // SAFETY: único acesso ao NVMC; o firmware é single-thread e sem interrupções.
    unsafe { &*pac::NVMC::ptr() }
}

fn wait_ready() {
    while nvmc().ready.read().ready().is_busy() {}
}

fn set_config(bits: u32) {
    nvmc().config.write(|w| unsafe { w.bits(bits) });
    wait_ready();
}

const REN: u32 = 0;
const WEN: u32 = 1;
const EEN: u32 = 2;

fn read_word(addr: u32) -> u32 {
    unsafe { core::ptr::read_volatile(addr as *const u32) }
}

fn write_word(addr: u32, value: u32) {
    unsafe { core::ptr::write_volatile(addr as *mut u32, value) };
    wait_ready();
}

pub fn read_uicr() -> [u32; UICR_WORDS] {
    let mut out = [0u32; UICR_WORDS];
    for (i, w) in out.iter_mut().enumerate() {
        *w = read_word(UICR_BASE + (i as u32) * 4);
    }
    out
}

pub fn read_backup() -> Backup {
    let mut words = [0u32; UICR_WORDS];
    for (i, w) in words.iter_mut().enumerate() {
        *w = read_word(BACKUP_ADDR + 4 + (i as u32) * 4);
    }
    Backup {
        magic: read_word(BACKUP_ADDR),
        words,
        crc: read_word(BACKUP_ADDR + 4 + (UICR_WORDS as u32) * 4),
    }
}

pub fn erase_page(addr: u32) {
    set_config(EEN);
    nvmc().erasepage().write(|w| unsafe { w.bits(addr) });
    wait_ready();
    set_config(REN);
}

pub fn write_backup(b: &Backup) {
    erase_page(BACKUP_ADDR);
    debug_assert!(BACKUP_WORDS as u32 * 4 <= PAGE_SIZE);
    set_config(WEN);
    write_word(BACKUP_ADDR, b.magic);
    for (i, &w) in b.words.iter().enumerate() {
        write_word(BACKUP_ADDR + 4 + (i as u32) * 4, w);
    }
    write_word(BACKUP_ADDR + 4 + (UICR_WORDS as u32) * 4, b.crc);
    set_config(REN);
}

pub fn erase_uicr() {
    set_config(EEN);
    nvmc().eraseuicr.write(|w| unsafe { w.bits(1) });
    wait_ready();
    set_config(REN);
}

pub fn write_uicr_word(idx: usize, value: u32) {
    set_config(WEN);
    write_word(UICR_BASE + (idx as u32) * 4, value);
    set_config(REN);
}
