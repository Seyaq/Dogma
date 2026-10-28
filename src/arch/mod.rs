//! Code spécifique à x86_64.
use core::arch::asm;

pub mod cpu;
pub mod gdt;
pub mod idt;
pub mod port;
pub mod serial;

/// Arrête définitivement le cœur courant.
pub fn halt_loop() -> ! {
    loop {
        // SAFETY: instructions privilégiées valides en ring 0 ; on ne revient jamais.
        unsafe { asm!("cli", "hlt", options(nomem, nostack)) };
    }
}

#[inline]
pub fn read_cr2() -> u64 {
    let v: u64;
    // SAFETY: lecture de CR2 sans effet de bord, ring 0.
    unsafe { asm!("mov {}, cr2", out(reg) v, options(nomem, nostack, preserves_flags)) };
    v
}
