#![no_std]
#![no_main]

mod flash;

use core::fmt::Write as _;
use core::mem::MaybeUninit;

use cortex_m_rt::entry;
use heapless::String;
use nrf52840_hal::clocks::Clocks;
use nrf52840_hal::pac;
use nrf52840_hal::usbd::{UsbPeripheral, Usbd};
use panic_halt as _;
use uicr_fix::logic::{
    crc32, expected_after_fix, fix_preconditions, parse_command, restore_plan, resume_needed, Backup,
    Command, FixCheck, RestorePlan, unpin_check, UnpinCheck, BOOTLOADER_ADDR, NRFFW0_IDX, PSELRESET_IDX,
    PSELRESET_UNPINNED, UICR_WORDS,
};
use usb_device::class_prelude::UsbBusAllocator;
use usb_device::device::{StringDescriptors, UsbDeviceBuilder, UsbVidPid};
use usbd_serial::{SerialPort, USB_CLASS_CDC};

type Out = String<4096>;
type Last = String<96>;

const NAMED: [(&str, usize); 8] = [
    ("NRFFW0", 5),
    ("NRFFW1", 6),
    ("PSELRESET0", 128),
    ("PSELRESET1", 129),
    ("APPROTECT", 130),
    ("NFCPINS", 131),
    ("DEBUGCTRL", 132),
    ("REGOUT0", 193),
];

fn report(out: &mut Out, last: &Last, resetreas: u32, p0_18: bool, p1_06: bool) {
    out.clear();
    let uicr = flash::read_uicr();
    let _ = write!(out, "\r\n--- uicr-fix ---\r\nRESETREAS=0x{:08X}\r\n", resetreas);
    for (name, idx) in NAMED {
        let _ = write!(out, "{}=0x{:08X}\r\n", name, uicr[idx]);
    }
    for (i, &w) in uicr.iter().enumerate() {
        if w != 0xFFFF_FFFF {
            let _ = write!(out, "+0x{:03X}=0x{:08X}\r\n", i * 4, w);
        }
    }
    let _ = write!(out, "P1.06={} P0.18={}\r\n", p1_06 as u8, p0_18 as u8);
    let valid = flash::read_backup().is_valid();
    let _ = write!(out, "BACKUP={}\r\n", if valid { "valido" } else { "invalido" });
    let _ = write!(out, "ULTIMO={}\r\n", last.as_str());
}

/// Acrescenta uma resposta ao buffer de saída (descarta se não couber).
fn say(out: &mut Out, msg: core::fmt::Arguments) {
    let _ = out.write_fmt(msg);
    let _ = out.push_str("\r\n");
}

/// Guarda o resultado do comando (mostrado em todo relatório) e também o responde na hora.
fn result(out: &mut Out, last: &mut Last, args: core::fmt::Arguments) {
    last.clear();
    let _ = last.write_fmt(args);
    say(out, format_args!("{}", last.as_str()));
}

fn verify(target: &[u32; UICR_WORDS]) -> Option<usize> {
    let now = flash::read_uicr();
    (0..UICR_WORDS).find(|&i| now[i] != target[i])
}

fn apply_plan(target: &[u32; UICR_WORDS]) -> Result<(), usize> {
    match restore_plan(target, &flash::read_uicr()) {
        RestorePlan::Done => Ok(()),
        RestorePlan::Write(writes) => {
            for (idx, value) in writes {
                flash::write_uicr_word(idx, value);
            }
            Ok(())
        }
        RestorePlan::Conflict(idx) => Err(idx),
    }
}

fn finish(out: &mut Out, last: &mut Last, target: &[u32; UICR_WORDS], plan: Result<(), usize>) {
    match plan {
        Err(idx) => result(out, last, format_args!("FIX FALHOU idx={}", idx)),
        Ok(()) => match verify(target) {
            None => result(out, last, format_args!("FIX OK, desplugue e plugue")),
            Some(i) => result(out, last, format_args!("FIX FALHOU idx={}", i)),
        },
    }
}

fn fix(out: &mut Out, last: &mut Last) {
    let old = flash::read_backup();
    if resume_needed(&old, &flash::read_uicr()) {
        // Retomada: o backup bom já existe; não regrava nem apaga.
        let target = expected_after_fix(&old.words);
        let plan = apply_plan(&target);
        finish(out, last, &target, plan);
        return;
    }
    let b = Backup::new(flash::read_uicr());
    flash::write_backup(&b);
    let rb = flash::read_backup();
    match fix_preconditions(&flash::read_uicr(), &rb) {
        FixCheck::Abort(m) => {
            result(out, last, format_args!("ABORT: {}", m));
            return;
        }
        FixCheck::NothingToDo => {
            result(out, last, format_args!("NADA A FAZER"));
            return;
        }
        FixCheck::Ok => {}
    }
    let target = expected_after_fix(&rb.words);
    flash::erase_uicr();
    let plan = apply_plan(&target);
    finish(out, last, &target, plan);
}

const DUMP_START: u32 = 0x000E_0000;
const DUMP_END: u32 = 0x0010_0000;
const DUMP_LINES_PER_FILL: usize = 32;

/// Leitura volátil de uma palavra da flash (somente leitura).
fn read_word(addr: u32) -> u32 {
    unsafe { core::ptr::read_volatile(addr as *const u32) }
}

/// Gera as próximas linhas do dump no buffer (que já está drenado). Devolve o próximo endereço,
/// ou None quando terminou (a linha final com o CRC já foi escrita).
fn dump_fill(out: &mut Out, last: &mut Last, mut addr: u32) -> Option<u32> {
    out.clear();
    for _ in 0..DUMP_LINES_PER_FILL {
        if addr >= DUMP_END {
            break;
        }
        let _ = write!(out, "D {:08X} ", addr);
        for i in 0..8 {
            for b in read_word(addr + 4 * i).to_le_bytes() {
                let _ = write!(out, "{:02X}", b);
            }
        }
        let _ = out.push_str("\r\n");
        addr += 32;
    }
    if addr < DUMP_END {
        return Some(addr);
    }
    let words = unsafe {
        core::slice::from_raw_parts(DUMP_START as *const u32, ((DUMP_END - DUMP_START) / 4) as usize)
    };
    let _ = write!(out, "DUMP FIM CRC={:08X}\r\n", crc32(words));
    last.clear();
    let _ = last.push_str("DUMP OK");
    None
}

fn unpin(out: &mut Out, last: &mut Last) {
    match unpin_check(&flash::read_uicr()) {
        UnpinCheck::Abort(m) => result(out, last, format_args!("ABORT: {}", m)),
        UnpinCheck::NothingToDo => result(out, last, format_args!("NADA A FAZER")),
        UnpinCheck::Write => {
            flash::write_uicr_word(PSELRESET_IDX[1], PSELRESET_UNPINNED);
            let got = flash::read_uicr()[PSELRESET_IDX[1]];
            if got == PSELRESET_UNPINNED {
                result(out, last, format_args!("UNPIN OK, desplugue e plugue"));
            } else {
                result(out, last, format_args!("UNPIN FALHOU leu=0x{:08X}", got));
            }
        }
    }
}

fn bootloader_configured() -> bool {
    flash::read_uicr()[NRFFW0_IDX] == BOOTLOADER_ADDR
}

#[entry]
fn main() -> ! {
    static mut CLOCKS: MaybeUninit<Clocks<nrf52840_hal::clocks::ExternalOscillator, nrf52840_hal::clocks::Internal, nrf52840_hal::clocks::LfOscStopped>> =
        MaybeUninit::uninit();
    static mut USB_BUS: MaybeUninit<UsbBusAllocator<Usbd<UsbPeripheral<'static>>>> =
        MaybeUninit::uninit();

    let p = pac::Peripherals::take().unwrap();
    let resetreas = p.POWER.resetreas.read().bits();
    p.POWER.resetreas.write(|w| unsafe { w.bits(resetreas) });

    // P1.06 e P0.18 como entrada com pull-up.
    p.P1.pin_cnf[6].write(|w| w.dir().input().input().connect().pull().pullup());
    p.P0.pin_cnf[18].write(|w| w.dir().input().input().connect().pull().pullup());

    // TIMER0 a 1 MHz como base de tempo.
    p.TIMER0.mode.write(|w| w.mode().timer());
    p.TIMER0.bitmode.write(|w| w.bitmode()._32bit());
    p.TIMER0.prescaler.write(|w| unsafe { w.prescaler().bits(4) });
    p.TIMER0.tasks_start.write(|w| unsafe { w.bits(1) });

    let clocks = CLOCKS.write(Clocks::new(p.CLOCK).enable_ext_hfosc());
    let usb_bus = UsbBusAllocator::new(Usbd::new(UsbPeripheral::new(p.USBD, clocks)));
    let usb_bus = USB_BUS.write(usb_bus);
    let mut serial = SerialPort::new(usb_bus);
    let mut usb_dev = UsbDeviceBuilder::new(usb_bus, UsbVidPid(0x1209, 0x0001))
        .strings(&[StringDescriptors::default().product("uicr-fix")])
        .unwrap()
        .device_class(USB_CLASS_CDC)
        .build();

    let mut out: Out = String::new();
    let mut sent = 0usize;
    let mut line = [0u8; 32];
    let mut len = 0usize;
    let mut tick = 0u32;
    let mut dump: Option<u32> = None; // próximo endereço do dump em andamento
    let mut last_result: Last = String::new();
    let _ = last_result.push_str("nenhum");

    loop {
        usb_dev.poll(&mut [&mut serial]);

        // Entrada: acumula até '\n'.
        let mut buf = [0u8; 32];
        if let Ok(n) = serial.read(&mut buf) {
            for &c in &buf[..n] {
                if c == b'\n' {
                    let cmd = parse_command(&line[..len]);
                    len = 0;
                    if dump.is_some() {
                        continue; // comandos durante o dump são ignorados
                    }
                    if sent >= out.len() {
                        out.clear();
                        sent = 0;
                    }
                    match cmd {
                        Command::Fix => fix(&mut out, &mut last_result),
                        Command::Dfu | Command::Wipe if !bootloader_configured() => result(
                            &mut out,
                            &mut last_result,
                            format_args!("RECUSADO: NRFFW0 apagado"),
                        ),
                        Command::Dfu => {
                            p.POWER.gpregret.write(|w| unsafe { w.bits(0xB1) });
                            cortex_m::peripheral::SCB::sys_reset();
                        }
                        Command::Wipe => {
                            flash::erase_page(0x1000);
                            cortex_m::peripheral::SCB::sys_reset();
                        }
                        Command::Dump => {
                            out.clear();
                            sent = 0;
                            dump = Some(DUMP_START);
                        }
                        Command::Unpin => unpin(&mut out, &mut last_result),
                        Command::Unknown => {
                            say(&mut out, format_args!("COMANDO? FIX|DFU|WIPE|DUMP|UNPIN"))
                        }
                    }
                } else if len < line.len() {
                    line[len] = c;
                    len += 1;
                }
            }
        }

        // Relatório uma vez por segundo (substitui o que ainda não foi enviado).
        p.TIMER0.tasks_capture[0].write(|w| unsafe { w.bits(1) });
        let now = p.TIMER0.cc[0].read().bits();
        // Nunca roda com saída ainda não enviada (sent < out.len()): report() faz out.clear() e
        // descartaria o resto do dump (inclusive a linha CRC) ou a resposta de um comando.
        if dump.is_none() && sent >= out.len() && now.wrapping_sub(tick) >= 1_000_000 {
            tick = now;
            let p0_18 = p.P0.in_.read().bits() & (1 << 18) != 0;
            let p1_06 = p.P1.in_.read().bits() & (1 << 6) != 0;
            report(&mut out, &last_result, resetreas, p0_18, p1_06);
            sent = 0;
        }

        // Dump: só gera mais linhas quando o buffer anterior foi inteiro para a serial.
        if let Some(addr) = dump {
            if sent >= out.len() {
                dump = dump_fill(&mut out, &mut last_result, addr);
                sent = 0;
                if dump.is_none() {
                    // O dump leva segundos: sem isto o tick velho dispararia um relatório logo após.
                    tick = now;
                }
            }
        }

        // Saída sem bloquear: WouldBlock apenas deixa o resto para a próxima volta.
        if sent < out.len() {
            if let Ok(n) = serial.write(&out.as_bytes()[sent..]) {
                sent += n;
            }
        }
    }
}
