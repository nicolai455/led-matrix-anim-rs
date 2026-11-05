use matrix_firmware::memory_diagnostics::{format_heap_snapshot, HeapRegion, HeapSnapshot};

#[test]
fn heap_snapshot_identifies_capabilities_and_reports_bytes_without_summing_them() {
    let snapshot = HeapSnapshot {
        internal: HeapRegion { free: 12000, minimum: 8000, largest: 6000 },
        dma: HeapRegion { free: 10000, minimum: 7000, largest: 4096 },
        psram: HeapRegion { free: 8000000, minimum: 7000000, largest: 6000000 },
    };
    let text = format_heap_snapshot("boot", snapshot);
    assert_eq!(text, "RAM stage=boot bytes internal(free=12000 min=8000 largest=6000) dma(free=10000 min=7000 largest=4096) psram(free=8000000 min=7000000 largest=6000000)");
}

#[test]
fn absent_psram_is_reported_as_zero_not_unavailable_memory() {
    let empty = HeapRegion { free: 0, minimum: 0, largest: 0 };
    assert!(format_heap_snapshot("periodic", HeapSnapshot { internal: empty, dma: empty, psram: empty })
        .contains("psram(free=0 min=0 largest=0)"));
}
