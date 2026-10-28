//! Console série COM1 (16550), sans verrou : un seul cœur actif à ce stade.
//! TODO(SMP) : verrou ou tampon par cœur avant de démarrer les autres CPU.
use super::port::{inb, outb};
use core::fmt::{self, Write};

const COM1: u16 = 0x3F8;

pub fn init() {
    // SAFETY: séquence d'initialisation standard du 16550 sur COM1.
    unsafe {
        outb(COM1 + 1, 0x00); // pas d'interruptions
        outb(COM1 + 3, 0x80); // DLAB on
        outb(COM1, 0x01); // diviseur = 1 → 115200 bauds
        outb(COM1 + 1, 0x00);
        outb(COM1 + 3, 0x03); // 8N1, DLAB off
        outb(COM1 + 2, 0xC7); // FIFO activé, vidé, seuil 14 octets
        outb(COM1 + 4, 0x0B); // DTR + RTS + OUT2
    }
}

fn write_byte(b: u8) {
    // SAFETY: lecture du LSR puis écriture du THR de COM1.
    unsafe {
        while inb(COM1 + 5) & 0x20 == 0 {
            core::hint::spin_loop();
        }
        outb(COM1, b);
    }
}

struct Serial;

impl Write for Serial {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for b in s.bytes() {
            if b == b'\n' {
                write_byte(b'\r');
            }
            write_byte(b);
        }
        Ok(())
    }
}

pub fn _print(args: fmt::Arguments) {
    let _ = Serial.write_fmt(args);
}

#[macro_export]
macro_rules! kprint {
    ($($arg:tt)*) => { $crate::arch::serial::_print(format_args!($($arg)*)) };
}

#[macro_export]
macro_rules! kprintln {
    () => { $crate::kprint!("\n") };
    ($($arg:tt)*) => { $crate::kprint!("{}\n", format_args!($($arg)*)) };
}
