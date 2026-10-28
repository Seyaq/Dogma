//! Allocateur de frames physiques : bitmap (1 = utilisé), recherche par mot de 64 bits
//! avec indice de départ → O(1) amorti, zéro allocation dynamique.
//! TODO(SMP) : listes libres par cœur + verrou fin ; pour l'instant un seul cœur.
use crate::kernel::boot::{self, MEMMAP_USABLE};
use crate::kprintln;
use core::ptr::null_mut;

pub const PAGE_SIZE: u64 = 4096;

struct Pmm {
    bitmap: *mut u64,
    words: usize,
    hint: usize,
    total_frames: u64,
    free_frames: u64,
}

static mut PMM: Pmm = Pmm { bitmap: null_mut(), words: 0, hint: 0, total_frames: 0, free_frames: 0 };

fn pmm() -> &'static mut Pmm {
    // SAFETY: un seul cœur et aucun accès depuis un handler d'interruption à ce stade.
    unsafe { &mut *(&raw mut PMM) }
}

impl Pmm {
    #[inline]
    fn word(&self, i: usize) -> u64 {
        debug_assert!(i < self.words);
        // SAFETY: i < words, bitmap valide après init().
        unsafe { self.bitmap.add(i).read() }
    }
    #[inline]
    fn set_word(&mut self, i: usize, v: u64) {
        debug_assert!(i < self.words);
        // SAFETY: idem.
        unsafe { self.bitmap.add(i).write(v) }
    }
    fn mark_used(&mut self, frame: u64) {
        let (w, b) = ((frame / 64) as usize, frame % 64);
        let v = self.word(w);
        self.set_word(w, v | (1 << b));
    }
    fn mark_free(&mut self, frame: u64) {
        let (w, b) = ((frame / 64) as usize, frame % 64);
        let v = self.word(w);
        self.set_word(w, v & !(1 << b));
    }
}

/// Construit la bitmap à partir de la carte mémoire Limine (régions USABLE uniquement ;
/// la mémoire « bootloader reclaimable » n'est pas récupérée pour l'instant).
pub fn init(hhdm: u64) {
    let mut top = 0u64;
    for e in boot::memmap().filter(|e| e.kind == MEMMAP_USABLE) {
        top = top.max(e.base + e.length);
    }
    let frames = (top / PAGE_SIZE) as usize;
    let words = frames.div_ceil(64);
    let bytes = (words * 8) as u64;

    let home = boot::memmap()
        .find(|e| e.kind == MEMMAP_USABLE && e.base >= 0x10_0000 && e.length >= bytes)
        .map(|e| e.base)
        .expect("aucune région USABLE assez grande pour la bitmap");

    let p = pmm();
    p.bitmap = (hhdm + home) as *mut u64;
    p.words = words;
    p.hint = 0;
    for i in 0..words {
        p.set_word(i, !0); // tout « utilisé » par défaut (trous, bits au-delà de `frames`)
    }

    let mut free = 0u64;
    for e in boot::memmap().filter(|e| e.kind == MEMMAP_USABLE) {
        for f in (e.base / PAGE_SIZE)..((e.base + e.length) / PAGE_SIZE) {
            p.mark_free(f);
            free += 1;
        }
    }
    // La bitmap elle-même et la frame 0 (adresse physique 0 = « nul ») restent réservées.
    p.mark_used(0);
    let (first, last) = (home / PAGE_SIZE, (home + bytes).div_ceil(PAGE_SIZE));
    for f in first..last {
        p.mark_used(f);
    }
    p.total_frames = frames as u64;
    p.free_frames = free - (last - first);
    kprintln!(
        "[dogma] PMM : {} MiB libres / {} MiB adressables (bitmap {} Kio)",
        p.free_frames * PAGE_SIZE >> 20,
        p.total_frames * PAGE_SIZE >> 20,
        bytes >> 10
    );
}

/// Alloue une frame de 4 Kio ; retourne son adresse physique.
pub fn alloc_frame() -> Option<u64> {
    let p = pmm();
    let mut i = p.hint;
    for _ in 0..p.words {
        let w = p.word(i);
        if w != !0 {
            let bit = (!w).trailing_zeros() as u64;
            p.set_word(i, w | (1 << bit));
            p.hint = i;
            p.free_frames -= 1;
            return Some((i as u64 * 64 + bit) * PAGE_SIZE);
        }
        i += 1;
        if i == p.words {
            i = 0;
        }
    }
    None
}

/// Libère une frame. Un double free est un bug noyau : on panique plutôt que de corrompre.
pub fn free_frame(phys: u64) {
    assert!(phys % PAGE_SIZE == 0, "free_frame: adresse non alignée");
    let p = pmm();
    let frame = phys / PAGE_SIZE;
    let (w, b) = ((frame / 64) as usize, frame % 64);
    assert!(w < p.words, "free_frame: hors de la bitmap");
    let v = p.word(w);
    assert!(v & (1 << b) != 0, "free_frame: double free de {phys:#x}");
    p.set_word(w, v & !(1 << b));
    p.hint = p.hint.min(w);
    p.free_frames += 1;
}

pub fn free_frames() -> u64 {
    pmm().free_frames
}
