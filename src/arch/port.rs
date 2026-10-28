//! Accès aux ports d'E/S.
use core::arch::asm;

/// # Safety
/// Écrire sur un port peut avoir des effets arbitraires sur le matériel.
#[inline]
pub unsafe fn outb(port: u16, value: u8) {
    unsafe {
        asm!("out dx, al", in("dx") port, in("al") value,
             options(nomem, nostack, preserves_flags));
    }
}

/// # Safety
/// Lire un port peut avoir des effets de bord sur le matériel.
#[inline]
pub unsafe fn inb(port: u16) -> u8 {
    let v: u8;
    unsafe {
        asm!("in al, dx", in("dx") port, out("al") v,
             options(nomem, nostack, preserves_flags));
    }
    v
}
