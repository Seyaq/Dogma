//! Registres de contrôle et MSR.
use core::arch::asm;

pub const MSR_EFER: u32 = 0xC000_0080;
pub const EFER_NXE: u64 = 1 << 11;
pub const CR0_WP: u64 = 1 << 16;

#[inline]
pub fn read_cr3() -> u64 {
    let v: u64;
    // SAFETY: lecture de CR3 sans effet de bord, ring 0.
    unsafe { asm!("mov {}, cr3", out(reg) v, options(nomem, nostack, preserves_flags)) };
    v
}

/// # Safety
/// `v` doit désigner une PML4 valide qui mappe le code courant, la pile et les données utilisées ensuite.
#[inline]
pub unsafe fn write_cr3(v: u64) {
    unsafe { asm!("mov cr3, {}", in(reg) v, options(nostack, preserves_flags)) };
}

#[inline]
pub fn read_cr0() -> u64 {
    let v: u64;
    // SAFETY: lecture de CR0, ring 0.
    unsafe { asm!("mov {}, cr0", out(reg) v, options(nomem, nostack, preserves_flags)) };
    v
}

/// # Safety
/// Modifier CR0 change le comportement global du CPU (paging, WP…).
#[inline]
pub unsafe fn write_cr0(v: u64) {
    unsafe { asm!("mov cr0, {}", in(reg) v, options(nomem, nostack, preserves_flags)) };
}

/// # Safety
/// `msr` doit exister sur ce CPU, sinon #GP.
#[inline]
pub unsafe fn rdmsr(msr: u32) -> u64 {
    let (lo, hi): (u32, u32);
    unsafe {
        asm!("rdmsr", in("ecx") msr, out("eax") lo, out("edx") hi,
             options(nomem, nostack, preserves_flags));
    }
    ((hi as u64) << 32) | lo as u64
}

/// # Safety
/// Écrire un MSR peut changer arbitrairement le comportement du CPU.
#[inline]
pub unsafe fn wrmsr(msr: u32, v: u64) {
    unsafe {
        asm!("wrmsr", in("ecx") msr, in("eax") v as u32, in("edx") (v >> 32) as u32,
             options(nomem, nostack, preserves_flags));
    }
}
