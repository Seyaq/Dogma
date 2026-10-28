//! Dogma — micro-noyau x86_64 : performance, légèreté, sécurité.
#![no_std]
#![no_main]

mod arch;
mod kernel;

use core::arch::asm;
use core::panic::PanicInfo;
use kernel::{boot, mem, paging};

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    arch::serial::init();
    kprintln!("[dogma] démarrage");

    if !boot::base_revision_supported() {
        kprintln!("[dogma] révision de base Limine non supportée");
        arch::halt_loop();
    }

    arch::gdt::init();
    arch::idt::init();
    kprintln!("[dogma] GDT/TSS/IDT chargées");

    let hhdm = boot::hhdm_offset().expect("Limine n'a pas fourni la HHDM");
    mem::init(hhdm);

    // Auto-tests de fumée : allocateur de frames puis exception #BP.
    let before = mem::free_frames();
    let a = mem::alloc_frame().expect("alloc a");
    let b = mem::alloc_frame().expect("alloc b");
    assert_ne!(a, b);
    mem::free_frame(a);
    mem::free_frame(b);
    assert_eq!(mem::free_frames(), before);

    // SAFETY: int3 vers notre handler #BP, qui retourne normalement.
    unsafe { asm!("int3") };

    paging::init(hhdm);
    paging::self_test();

    // SAFETY: on est déjà passé par #BP avec l'IDT chargée ; la nouvelle PML4 mappe le noyau.
    unsafe { asm!("int3") };

    kprintln!("[dogma] boot ok");
    arch::halt_loop()
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    kprintln!("\n[dogma] PANIC : {info}");
    arch::halt_loop()
}
