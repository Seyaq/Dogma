//! IDT et gestion des exceptions CPU (vecteurs 0..31).
use super::gdt::{DOUBLE_FAULT_IST, KERNEL_CS};
use crate::kprintln;
use core::arch::{asm, global_asm};
use core::mem::size_of;

#[repr(C)]
#[derive(Clone, Copy)]
struct Entry {
    offset_low: u16,
    selector: u16,
    ist: u8,
    attr: u8,
    offset_mid: u16,
    offset_high: u32,
    zero: u32,
}

impl Entry {
    const MISSING: Self = Self { offset_low: 0, selector: 0, ist: 0, attr: 0, offset_mid: 0, offset_high: 0, zero: 0 };

    fn interrupt_gate(handler: usize, ist: u8) -> Self {
        let h = handler as u64;
        Self {
            offset_low: h as u16,
            selector: KERNEL_CS,
            ist,
            attr: 0x8E, // présent, DPL 0, interrupt gate (IF effacé à l'entrée)
            offset_mid: (h >> 16) as u16,
            offset_high: (h >> 32) as u32,
            zero: 0,
        }
    }
}

#[repr(C, packed)]
struct DescriptorPointer {
    limit: u16,
    base: u64,
}

/// Disposition identique à l'ordre des push dans `isr_common`.
#[repr(C)]
pub struct ExceptionFrame {
    r15: u64, r14: u64, r13: u64, r12: u64, r11: u64, r10: u64, r9: u64, r8: u64,
    rbp: u64, rdi: u64, rsi: u64, rdx: u64, rcx: u64, rbx: u64, rax: u64,
    vector: u64,
    error: u64,
    rip: u64,
    cs: u64,
    rflags: u64,
    rsp: u64,
    ss: u64,
}

static mut IDT: [Entry; 256] = [Entry::MISSING; 256];

unsafe extern "C" {
    static isr_table: [usize; 32];
}

// Les vecteurs 8, 10-14, 17, 21, 29, 30 reçoivent un code d'erreur du CPU ;
// pour les autres on pousse 0 afin d'avoir une trame uniforme.
global_asm!(
    r#"
    .macro ISR_NOERR n
    isr_stub_\n:
        push 0
        push \n
        jmp isr_common
    .endm
    .macro ISR_ERR n
    isr_stub_\n:
        push \n
        jmp isr_common
    .endm

    .section .text
    .irp n, 0,1,2,3,4,5,6,7
        ISR_NOERR \n
    .endr
    ISR_ERR 8
    ISR_NOERR 9
    .irp n, 10,11,12,13,14
        ISR_ERR \n
    .endr
    ISR_NOERR 15
    ISR_NOERR 16
    ISR_ERR 17
    .irp n, 18,19,20
        ISR_NOERR \n
    .endr
    ISR_ERR 21
    .irp n, 22,23,24,25,26,27,28
        ISR_NOERR \n
    .endr
    ISR_ERR 29
    ISR_ERR 30
    ISR_NOERR 31

    isr_common:
        push rax
        push rbx
        push rcx
        push rdx
        push rsi
        push rdi
        push rbp
        push r8
        push r9
        push r10
        push r11
        push r12
        push r13
        push r14
        push r15
        mov rdi, rsp
        cld
        call exception_handler
        pop r15
        pop r14
        pop r13
        pop r12
        pop r11
        pop r10
        pop r9
        pop r8
        pop rbp
        pop rdi
        pop rsi
        pop rdx
        pop rcx
        pop rbx
        pop rax
        add rsp, 16
        iretq

    .section .rodata
    .global isr_table
    .balign 8
    isr_table:
    .irp n, 0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23,24,25,26,27,28,29,30,31
        .quad isr_stub_\n
    .endr
    .section .text
    "#
);

const NAMES: [&str; 32] = [
    "#DE divide error", "#DB debug", "NMI", "#BP breakpoint", "#OF overflow",
    "#BR bound range", "#UD invalid opcode", "#NM device not available",
    "#DF double fault", "coprocessor overrun", "#TS invalid TSS",
    "#NP segment not present", "#SS stack fault", "#GP general protection",
    "#PF page fault", "reserved", "#MF x87", "#AC alignment check",
    "#MC machine check", "#XM SIMD", "#VE virtualization", "#CP control protection",
    "reserved", "reserved", "reserved", "reserved", "reserved", "reserved",
    "#HV hypervisor", "#VC VMM communication", "#SX security", "reserved",
];

#[unsafe(no_mangle)]
pub extern "C" fn exception_handler(f: &mut ExceptionFrame) {
    if f.vector == 3 {
        kprintln!("[dogma] #BP à {:#x} (retour normal)", f.rip);
        return;
    }
    kprintln!("\n[dogma] EXCEPTION {} (vecteur {}), erreur={:#x}", NAMES[f.vector as usize], f.vector, f.error);
    kprintln!("  rip={:#x} cs={:#x} rflags={:#x} rsp={:#x} ss={:#x}", f.rip, f.cs, f.rflags, f.rsp, f.ss);
    if f.vector == 14 {
        kprintln!("  cr2={:#x}", super::read_cr2());
    }
    kprintln!("  rax={:#x} rbx={:#x} rcx={:#x} rdx={:#x}", f.rax, f.rbx, f.rcx, f.rdx);
    super::halt_loop();
}

pub fn init() {
    // SAFETY: appelé une fois sur un seul cœur, interruptions masquées ; `isr_table`
    // est une table de 32 adresses valides définie dans le global_asm ci-dessus.
    unsafe {
        let idt = (&raw mut IDT).cast::<Entry>();
        for v in 0..32usize {
            let ist = if v == 8 { DOUBLE_FAULT_IST } else { 0 };
            idt.add(v).write(Entry::interrupt_gate(isr_table[v], ist));
        }
        let ptr = DescriptorPointer {
            limit: (size_of::<[Entry; 256]>() - 1) as u16,
            base: idt as u64,
        };
        asm!("lidt [{}]", in(reg) &raw const ptr, options(readonly, nostack, preserves_flags));
    }
}
