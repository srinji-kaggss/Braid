use braid_verify::decode::{MAX_VALUE_NODES, decode_canonical, preflight_canonical};
use std::alloc::{GlobalAlloc, Layout, System};
use std::hint::black_box;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Instant;

struct CountingAllocator;

static ENABLED: AtomicBool = AtomicBool::new(false);
static ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);
static TOTAL_BYTES: AtomicUsize = AtomicUsize::new(0);
static LIVE_BYTES: AtomicUsize = AtomicUsize::new(0);
static PEAK_LIVE_BYTES: AtomicUsize = AtomicUsize::new(0);

fn record_allocation(size: usize) {
    ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
    TOTAL_BYTES.fetch_add(size, Ordering::Relaxed);
    let live = LIVE_BYTES.fetch_add(size, Ordering::Relaxed) + size;
    PEAK_LIVE_BYTES.fetch_max(live, Ordering::Relaxed);
}

fn record_deallocation(size: usize) {
    let mut live = LIVE_BYTES.load(Ordering::Relaxed);
    loop {
        let next = live.saturating_sub(size);
        match LIVE_BYTES.compare_exchange_weak(live, next, Ordering::Relaxed, Ordering::Relaxed) {
            Ok(_) => break,
            Err(observed) => live = observed,
        }
    }
}

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: forwarding the exact layout to the process System allocator.
        let pointer = unsafe { System.alloc(layout) };
        if ENABLED.load(Ordering::Relaxed) && !pointer.is_null() {
            record_allocation(layout.size());
        }
        pointer
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        // SAFETY: forwarding the exact layout to the process System allocator.
        let pointer = unsafe { System.alloc_zeroed(layout) };
        if ENABLED.load(Ordering::Relaxed) && !pointer.is_null() {
            record_allocation(layout.size());
        }
        pointer
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        if ENABLED.load(Ordering::Relaxed) {
            record_deallocation(layout.size());
        }
        // SAFETY: `pointer` and `layout` are the pair supplied by Rust for the
        // allocation originally delegated to System.
        unsafe { System.dealloc(pointer, layout) };
    }

    unsafe fn realloc(&self, pointer: *mut u8, old: Layout, new_size: usize) -> *mut u8 {
        // SAFETY: forwarding the original pointer/layout pair and requested
        // size unchanged to the process System allocator.
        let replacement = unsafe { System.realloc(pointer, old, new_size) };
        if ENABLED.load(Ordering::Relaxed) && !replacement.is_null() {
            record_deallocation(old.size());
            record_allocation(new_size);
        }
        replacement
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

fn reset() {
    ALLOCATIONS.store(0, Ordering::Relaxed);
    TOTAL_BYTES.store(0, Ordering::Relaxed);
    LIVE_BYTES.store(0, Ordering::Relaxed);
    PEAK_LIVE_BYTES.store(0, Ordering::Relaxed);
}

fn measure(label: &str, iterations: usize, mut operation: impl FnMut()) {
    reset();
    ENABLED.store(true, Ordering::SeqCst);
    let started = Instant::now();
    for _ in 0..iterations {
        operation();
    }
    let elapsed = started.elapsed();
    ENABLED.store(false, Ordering::SeqCst);

    println!(
        "{label} iterations={iterations} allocations={} total_bytes={} peak_live_bytes={} elapsed_us={}",
        ALLOCATIONS.load(Ordering::Relaxed),
        TOTAL_BYTES.load(Ordering::Relaxed),
        PEAK_LIVE_BYTES.load(Ordering::Relaxed),
        elapsed.as_micros()
    );
}

fn hostile_value_nodes() -> Vec<u8> {
    let mut bytes = vec![0x9a];
    bytes.extend_from_slice(&(MAX_VALUE_NODES as u32).to_be_bytes());
    bytes.resize(bytes.len() + MAX_VALUE_NODES as usize, 0xf4);
    bytes
}

fn main() {
    let accepted = braid_vocab_cms::edit_section_capsule().to_bytes();
    let rejected = hostile_value_nodes();

    measure("accepted_preflight", 10_000, || {
        black_box(preflight_canonical(black_box(&accepted))).unwrap();
    });
    measure("rejected_preflight", 100, || {
        black_box(preflight_canonical(black_box(&rejected))).unwrap_err();
    });
    measure("accepted_owned_decode", 1_000, || {
        drop(black_box(decode_canonical(black_box(&accepted))).unwrap());
    });
    measure("rejected_owned_decode", 100, || {
        black_box(decode_canonical(black_box(&rejected))).unwrap_err();
    });
}
