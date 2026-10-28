//! Protocole de boot Limine (révision de base 3), écrit à la main : zéro dépendance.
//!
//! ⚠ Les identifiants de requêtes et la disposition des structures sont écrits
//! d'après la spécification Limine (PROTOCOL.md) et n'ont PAS encore été validés
//! sur un boot réel : à vérifier contre la version de Limine utilisée.
use core::cell::UnsafeCell;
use core::ptr::{null, read_volatile};

const COMMON_MAGIC: [u64; 2] = [0xc7b1dd30df4c8b88, 0x0a82e883a194f07b];

/// Le chargeur met `[2]` à 0 s'il supporte la révision demandée.
#[repr(transparent)]
struct BaseRevision(UnsafeCell<[u64; 3]>);
// SAFETY: écrit uniquement par le chargeur avant l'entrée du noyau.
unsafe impl Sync for BaseRevision {}

#[used]
#[unsafe(link_section = ".limine_requests")]
static BASE_REVISION: BaseRevision =
    BaseRevision(UnsafeCell::new([0xf9562b2d5c95a6c8, 0x6a7b384944536bdc, 3]));

#[repr(C)]
struct Request<T> {
    id: [u64; 4],
    revision: u64,
    response: UnsafeCell<*const T>,
}
// SAFETY: `response` n'est écrit que par le chargeur avant l'entrée du noyau.
unsafe impl<T> Sync for Request<T> {}

impl<T> Request<T> {
    const fn new(id3: u64, id4: u64) -> Self {
        Self {
            id: [COMMON_MAGIC[0], COMMON_MAGIC[1], id3, id4],
            revision: 0,
            response: UnsafeCell::new(null()),
        }
    }

    fn response(&self) -> Option<&'static T> {
        // SAFETY: lecture volatile (le chargeur a écrit hors de la vue du compilateur) ;
        // le pointeur est soit nul, soit une réponse valide en mémoire bootloader-reclaimable
        // que l'on ne libère jamais.
        unsafe { read_volatile(self.response.get()).as_ref() }
    }
}

#[repr(C)]
struct HhdmResponse {
    revision: u64,
    offset: u64,
}

#[repr(C)]
struct MemmapResponse {
    revision: u64,
    entry_count: u64,
    entries: *const *const MemmapEntry,
}

#[repr(C)]
pub struct MemmapEntry {
    pub base: u64,
    pub length: u64,
    pub kind: u64,
}

pub const MEMMAP_USABLE: u64 = 0;
#[allow(dead_code)]
pub const MEMMAP_BOOTLOADER_RECLAIMABLE: u64 = 5;

#[used]
#[unsafe(link_section = ".limine_requests")]
static HHDM: Request<HhdmResponse> = Request::new(0x48dcf1cb8ad2b852, 0x63984e959a98244b);

#[used]
#[unsafe(link_section = ".limine_requests")]
static MEMMAP: Request<MemmapResponse> = Request::new(0x67cf3d9d378a806f, 0xe304acdfc50c3c62);

pub fn base_revision_supported() -> bool {
    // SAFETY: lecture volatile d'un mot que le chargeur a pu modifier.
    unsafe { read_volatile(&raw const (*BASE_REVISION.0.get())[2]) == 0 }
}

/// Décalage de la HHDM : adresse virtuelle = adresse physique + offset.
pub fn hhdm_offset() -> Option<u64> {
    HHDM.response().map(|r| r.offset)
}

pub fn memmap() -> impl Iterator<Item = &'static MemmapEntry> {
    let (n, entries) = match MEMMAP.response() {
        Some(r) => (r.entry_count as usize, r.entries),
        None => (0, null()),
    };
    // SAFETY: Limine garantit `entry_count` pointeurs valides dans `entries`.
    (0..n).map(move |i| unsafe { &**entries.add(i) })
}
