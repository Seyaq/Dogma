//! Tables de pages du noyau (PML4 propre, remplace celle de Limine).
//!
//! Politique :
//! - HHDM reconstruite uniquement pour USABLE, ACPI reclaimable et bootloader reclaimable
//!   (RW+NX) ; pages 2 Mio quand l'alignement le permet, 4 Kio sinon.
//! - L'image du noyau n'est PAS mappée dans la HHDM : pas d'alias inscriptible du code (W^X).
//! - Image noyau en moitié haute : .text = R-X, .rodata = R--+NX, .data/.bss = RW+NX.
//! - EFER.NXE et CR0.WP activés avant le changement de CR3.
use crate::arch::cpu;
use crate::kernel::{boot, mem};
use crate::kprintln;

const P: u64 = 1;
const W: u64 = 1 << 1;
const PS: u64 = 1 << 7;
const NX: u64 = 1 << 63;
const ADDR_MASK: u64 = 0x000f_ffff_ffff_f000;
const MIB2: u64 = 2 << 20;
const PAGE: u64 = 4096;

const KIND_USABLE: u64 = 0;
const KIND_ACPI_RECLAIMABLE: u64 = 2;
const KIND_BOOTLOADER_RECLAIMABLE: u64 = 5;

static mut HHDM: u64 = 0;

unsafe extern "C" {
    static __text_start: u8;
    static __text_end: u8;
    static __rodata_start: u8;
    static __rodata_end: u8;
    static __data_start: u8;
    static __data_end: u8;
}

#[inline]
fn hhdm() -> u64 {
    // SAFETY: écrit une seule fois dans init(), avant tout usage.
    unsafe { HHDM }
}

#[inline]
fn table(phys: u64) -> *mut u64 {
    (hhdm() + phys) as *mut u64
}

#[inline]
fn idx(virt: u64, shift: u32) -> usize {
    ((virt >> shift) & 511) as usize
}

fn new_table() -> u64 {
    let f = mem::alloc_frame().expect("paging : plus de frames pour les tables");
    // SAFETY: la frame vient d'être allouée et est atteignable via la HHDM courante.
    unsafe { core::ptr::write_bytes(table(f), 0, 512) };
    f
}

/// Descend d'un niveau en créant la table intermédiaire si besoin.
fn next_level(table_phys: u64, index: usize) -> u64 {
    // SAFETY: table_phys est une table valide, index < 512.
    unsafe {
        let slot = table(table_phys).add(index);
        let e = slot.read();
        if e & P != 0 {
            assert!(e & PS == 0, "paging : page huge sur le chemin d'un mapping 4K");
            return e & ADDR_MASK;
        }
        let t = new_table();
        slot.write(t | P | W); // les droits effectifs sont ceux de la feuille
        t
    }
}

fn map_4k(root: u64, virt: u64, phys: u64, flags: u64) {
    let pdpt = next_level(root, idx(virt, 39));
    let pd = next_level(pdpt, idx(virt, 30));
    let pt = next_level(pd, idx(virt, 21));
    // SAFETY: pt est une table valide.
    unsafe { table(pt).add(idx(virt, 12)).write(phys | flags | P) };
}

fn map_2m(root: u64, virt: u64, phys: u64, flags: u64) {
    let pdpt = next_level(root, idx(virt, 39));
    let pd = next_level(pdpt, idx(virt, 30));
    // SAFETY: pd est une table valide.
    unsafe { table(pd).add(idx(virt, 21)).write(phys | flags | P | PS) };
}

/// Mappe [start, end) physique dans la HHDM.
fn map_hhdm_range(root: u64, start: u64, end: u64, flags: u64) {
    let mut p = start;
    while p < end {
        if p % MIB2 == 0 && end - p >= MIB2 {
            map_2m(root, hhdm() + p, p, flags);
            p += MIB2;
        } else {
            map_4k(root, hhdm() + p, p, flags);
            p += PAGE;
        }
    }
}

/// Parcourt les tables `root` ; retourne (adresse physique, entrée feuille).
pub fn translate(root: u64, virt: u64) -> Option<(u64, u64)> {
    let mut t = root & ADDR_MASK;
    for shift in [39u32, 30, 21, 12] {
        // SAFETY: t est une table valide (racine ou lue dans une entrée présente).
        let e = unsafe { table(t).add(idx(virt, shift)).read() };
        if e & P == 0 {
            return None;
        }
        if shift == 12 {
            return Some(((e & ADDR_MASK) | (virt & 0xFFF), e));
        }
        if shift != 39 && e & PS != 0 {
            let mask = (1u64 << shift) - 1;
            return Some((((e & ADDR_MASK) & !mask) | (virt & mask), e));
        }
        t = e & ADDR_MASK;
    }
    None
}

/// Mappe une section de l'image noyau en réutilisant les adresses physiques de l'ancienne PML4.
fn map_section(root: u64, old_root: u64, start: u64, end: u64, flags: u64) {
    let end = (end + PAGE - 1) & !(PAGE - 1);
    let mut v = start;
    while v < end {
        let (phys, _) = translate(old_root, v).expect("paging : section noyau non mappée par Limine");
        map_4k(root, v, phys & !(PAGE - 1), flags);
        v += PAGE;
    }
}

fn sym(p: *const u8) -> u64 {
    p as u64
}

pub fn init(hhdm_offset: u64) {
    // SAFETY: une seule écriture, avant toute lecture.
    unsafe { HHDM = hhdm_offset };
    let old_root = cpu::read_cr3() & ADDR_MASK;
    let root = new_table();

    for e in boot::memmap() {
        match e.kind {
            KIND_USABLE | KIND_ACPI_RECLAIMABLE | KIND_BOOTLOADER_RECLAIMABLE => {
                map_hhdm_range(root, e.base, e.base + e.length, W | NX);
            }
            _ => {} // réservé, NVS, mauvais, framebuffer, image noyau : volontairement non mappé
        }
    }

    // SAFETY: symboles définis par linker.ld ; on ne prend que leurs adresses.
    let (ts, te, rs, re, ds, de) = unsafe {
        (
            sym(&raw const __text_start),
            sym(&raw const __text_end),
            sym(&raw const __rodata_start),
            sym(&raw const __rodata_end),
            sym(&raw const __data_start),
            sym(&raw const __data_end),
        )
    };
    map_section(root, old_root, ts, te, 0); // R-X
    map_section(root, old_root, rs, re, NX); // R--
    map_section(root, old_root, ds, de, W | NX); // RW-

    // SAFETY: NXE doit être actif avant de charger des tables contenant des bits NX ;
    // WP impose les pages en lecture seule aussi au ring 0 ; la nouvelle PML4 mappe
    // le noyau, la HHDM (donc la pile Limine) et les données utilisées ensuite.
    unsafe {
        cpu::wrmsr(cpu::MSR_EFER, cpu::rdmsr(cpu::MSR_EFER) | cpu::EFER_NXE);
        cpu::write_cr0(cpu::read_cr0() | cpu::CR0_WP);
        cpu::write_cr3(root);
    }
    kprintln!("[dogma] paging : PML4 noyau active ({:#x}), NX + WP", root);
}

/// Vérifie les propriétés de sécurité attendues ; panique en cas d'écart.
pub fn self_test() {
    let root = cpu::read_cr3() & ADDR_MASK;
    // SAFETY: adresses de symboles du linker script.
    let (ts, rs, ds) = unsafe {
        (sym(&raw const __text_start), sym(&raw const __rodata_start), sym(&raw const __data_start))
    };

    let (ktext_phys, e_text) = translate(root, ts).expect("text non mappé");
    assert!(e_text & NX == 0 && e_text & W == 0, "text doit être R-X");
    let (_, e_ro) = translate(root, rs).expect("rodata non mappé");
    assert!(e_ro & NX != 0 && e_ro & W == 0, "rodata doit être R-- NX");
    let (_, e_data) = translate(root, ds).expect("data non mappé");
    assert!(e_data & NX != 0 && e_data & W != 0, "data doit être RW- NX");

    assert!(
        translate(root, hhdm() + (ktext_phys & !(PAGE - 1))).is_none(),
        "le code noyau est atteignable en écriture via la HHDM"
    );

    let f = mem::alloc_frame().expect("alloc test");
    let p = (hhdm() + f) as *mut u64;
    // SAFETY: frame fraîchement allouée, mappée RW dans la HHDM.
    unsafe {
        p.write_volatile(0xD06A_D06A_D06A_D06A);
        assert_eq!(p.read_volatile(), 0xD06A_D06A_D06A_D06A);
    }
    mem::free_frame(f);
    kprintln!("[dogma] paging : W^X vérifié, HHDM sans alias du code noyau");
}
