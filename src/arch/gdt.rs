//! GDT + TSS. Ordre des sélecteurs imposé par SYSCALL/SYSRET (STAR) :
//! 0x08 kcode, 0x10 kdata, 0x18 udata, 0x20 ucode, 0x28 TSS.
use core::arch::asm;
use core::mem::size_of;

pub const KERNEL_CS: u16 = 0x08;
pub const KERNEL_DS: u16 = 0x10;
#[allow(dead_code)]
pub const USER_DS: u16 = 0x18 | 3;
#[allow(dead_code)]
pub const USER_CS: u16 = 0x20 | 3;
const TSS_SEL: u16 = 0x28;
/// Index IST (1-based) utilisé par #DF : pile dédiée, survit à une pile noyau corrompue.
pub const DOUBLE_FAULT_IST: u8 = 1;

#[repr(C, packed)]
struct Tss {
    reserved0: u32,
    rsp: [u64; 3],
    reserved1: u64,
    ist: [u64; 7],
    reserved2: u64,
    reserved3: u16,
    iopb_offset: u16,
}

#[repr(C, align(16))]
struct Stack([u8; 16 * 1024]);

#[repr(C, packed)]
struct DescriptorPointer {
    limit: u16,
    base: u64,
}

static mut DF_STACK: Stack = Stack([0; 16 * 1024]);
static mut TSS: Tss = Tss {
    reserved0: 0,
    rsp: [0; 3],
    reserved1: 0,
    ist: [0; 7],
    reserved2: 0,
    reserved3: 0,
    // Offset >= limite du TSS : aucune bitmap d'E/S (ring 3 interdit d'accéder aux ports).
    iopb_offset: size_of::<Tss>() as u16,
};
static mut GDT: [u64; 7] = [0; 7];

pub fn init() {
    // SAFETY: appelé une seule fois, très tôt, sur un seul cœur ; les statiques
    // ne sont accédées que par pointeurs bruts.
    unsafe {
        let tss = &raw mut TSS;
        let df_top = (&raw const DF_STACK as u64) + size_of::<Stack>() as u64;
        (&raw mut (*tss).ist).cast::<u64>().write_unaligned(df_top); // IST1

        let base = tss as u64;
        let limit = (size_of::<Tss>() - 1) as u64;
        let tss_low = (limit & 0xFFFF)
            | ((base & 0xFFFF) << 16)
            | (((base >> 16) & 0xFF) << 32)
            | (0x89u64 << 40) // présent, TSS 64-bit disponible
            | (((limit >> 16) & 0xF) << 48)
            | (((base >> 24) & 0xFF) << 56);
        let tss_high = base >> 32;

        let gdt = (&raw mut GDT).cast::<u64>();
        gdt.add(0).write(0);
        gdt.add(1).write(0x00AF_9A00_0000_FFFF); // code noyau 64-bit
        gdt.add(2).write(0x00CF_9200_0000_FFFF); // données noyau
        gdt.add(3).write(0x00CF_F200_0000_FFFF); // données utilisateur
        gdt.add(4).write(0x00AF_FA00_0000_FFFF); // code utilisateur 64-bit
        gdt.add(5).write(tss_low);
        gdt.add(6).write(tss_high);

        let ptr = DescriptorPointer {
            limit: (size_of::<[u64; 7]>() - 1) as u16,
            base: gdt as u64,
        };
        asm!("lgdt [{}]", in(reg) &raw const ptr, options(readonly, nostack, preserves_flags));

        // Recharge CS par retour lointain, puis les sélecteurs de données.
        asm!(
            "push {cs}",
            "lea {tmp}, [rip + 2f]",
            "push {tmp}",
            "retfq",
            "2:",
            "mov ds, {ds:x}",
            "mov es, {ds:x}",
            "mov ss, {ds:x}",
            "xor {tmp:e}, {tmp:e}",
            "mov fs, {tmp:x}",
            "mov gs, {tmp:x}",
            cs = const KERNEL_CS as u64,
            ds = in(reg) KERNEL_DS,
            tmp = out(reg) _,
            options(preserves_flags),
        );

        asm!("ltr {0:x}", in(reg) TSS_SEL, options(nomem, nostack, preserves_flags));
    }
}
