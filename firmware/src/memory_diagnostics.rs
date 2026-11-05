//! Heap diagnostics; capability regions overlap and must not be added together.
use std::fmt::{self, Write};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HeapRegion {
    pub free: usize,
    pub minimum: usize,
    pub largest: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HeapSnapshot {
    pub internal: HeapRegion,
    pub dma: HeapRegion,
    pub psram: HeapRegion,
}

pub fn write_heap_snapshot(out: &mut impl Write, stage: &str, snapshot: HeapSnapshot) -> fmt::Result {
    let HeapSnapshot { internal, dma, psram } = snapshot;
    write!(out, "RAM stage={stage} bytes internal(free={} min={} largest={}) dma(free={} min={} largest={}) psram(free={} min={} largest={})",
        internal.free, internal.minimum, internal.largest,
        dma.free, dma.minimum, dma.largest,
        psram.free, psram.minimum, psram.largest)
}

pub fn format_heap_snapshot(stage: &str, snapshot: HeapSnapshot) -> String {
    let mut text = String::new();
    write_heap_snapshot(&mut text, stage, snapshot).expect("writing to String cannot fail");
    text
}

#[cfg(target_os = "espidf")]
pub fn log_heap(stage: &str) {
    use esp_idf_sys as idf;
    fn region(caps: u32) -> HeapRegion {
        unsafe {
            HeapRegion {
                free: idf::heap_caps_get_free_size(caps),
                minimum: idf::heap_caps_get_minimum_free_size(caps),
                largest: idf::heap_caps_get_largest_free_block(caps),
            }
        }
    }
    let snapshot = HeapSnapshot {
        internal: region(idf::MALLOC_CAP_INTERNAL | idf::MALLOC_CAP_8BIT),
        dma: region(idf::MALLOC_CAP_INTERNAL | idf::MALLOC_CAP_DMA),
        psram: region(idf::MALLOC_CAP_SPIRAM | idf::MALLOC_CAP_8BIT),
    };
    struct DisplaySnapshot<'a>(&'a str, HeapSnapshot);
    impl fmt::Display for DisplaySnapshot<'_> {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write_heap_snapshot(f, self.0, self.1)
        }
    }
    log::warn!("{}", DisplaySnapshot(stage, snapshot));
}

/// Called from the task itself, so no task handle can become stale.
#[cfg(target_os = "espidf")]
pub fn log_current_stack(task: &str) {
    let free = unsafe { esp_idf_sys::uxTaskGetStackHighWaterMark(std::ptr::null_mut()) };
    log::warn!("RAM stack task={task} minimum_free_bytes={free}");
}
