use matrix_firmware::net::http::{
    api_admission, shed_dynamic_api, ApiAdmission, HttpHeapPressure, DMA_PRESSURE_FLOOR_BYTES,
    MAX_API_BODY_BYTES,
};

#[test]
fn cached_gets_and_status_remain_available_under_pressure() {
    for path in [
        "/api/state",
        "/api/effects",
        "/api/catalog",
        "/api/status",
        "/api/firmware",
        "/api/recovery",
    ] {
        assert_eq!(
            api_admission("GET", path, true),
            ApiAdmission::Allow,
            "{path}"
        );
    }
}

#[test]
fn pressure_sheds_dynamic_routes_but_not_recovery_or_streaming_ota() {
    for (method, path) in [
        ("GET", "/api/frame"),
        ("GET", "/api/audio"),
        ("GET", "/api/effects/state"),
        ("POST", "/api/state"),
        ("POST", "/api/config"),
        ("POST", "/api/unknown"),
    ] {
        assert_eq!(
            api_admission(method, path, true),
            ApiAdmission::Shed,
            "{method} {path}"
        );
        assert_eq!(
            api_admission(method, path, false),
            ApiAdmission::Allow,
            "{method} {path}"
        );
    }
    for (method, path) in [
        ("POST", "/api/recovery/retry"),
        ("POST", "/api/reboot"),
        ("POST", "/update"),
        ("GET", "/"),
        ("GET", "/preview.wasm.gz"),
    ] {
        assert_eq!(
            api_admission(method, path, true),
            ApiAdmission::Allow,
            "{method} {path}"
        );
    }
}

fn at_floors() -> HttpHeapPressure {
    HttpHeapPressure {
        internal_free: MAX_API_BODY_BYTES,
        internal_largest: MAX_API_BODY_BYTES,
        dma_free: DMA_PRESSURE_FLOOR_BYTES,
        dma_largest: DMA_PRESSURE_FLOOR_BYTES,
    }
}

#[test]
fn accepts_at_and_above_each_floor() {
    assert!(!shed_dynamic_api(at_floors()));
    assert!(!shed_dynamic_api(HttpHeapPressure {
        internal_free: usize::MAX,
        internal_largest: usize::MAX,
        dma_free: usize::MAX,
        dma_largest: usize::MAX,
    }));
}

#[test]
fn each_capability_and_contiguous_block_is_independently_required() {
    let base = at_floors();
    for heap in [
        HttpHeapPressure {
            internal_free: MAX_API_BODY_BYTES - 1,
            ..base
        },
        HttpHeapPressure {
            internal_largest: MAX_API_BODY_BYTES - 1,
            ..base
        },
        HttpHeapPressure {
            dma_free: DMA_PRESSURE_FLOOR_BYTES - 1,
            ..base
        },
        HttpHeapPressure {
            dma_largest: DMA_PRESSURE_FLOOR_BYTES - 1,
            ..base
        },
    ] {
        assert!(shed_dynamic_api(heap), "{heap:?}");
    }
}

#[test]
fn zero_memory_is_rejected() {
    assert!(shed_dynamic_api(HttpHeapPressure {
        internal_free: 0,
        internal_largest: 0,
        dma_free: 0,
        dma_largest: 0,
    }));
}
