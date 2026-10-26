use matrix_core::Engine;
use matrix_effects::builtin_effects;
use serde_json::Value;

#[test]
fn test_legacy_catalog_full_parity() {
    let effects = builtin_effects();
    let engine = Engine::new(effects).expect("Failed to create engine with builtin effects");
    let catalog = engine.catalog();
    let entries = catalog.as_array().expect("Catalog must be an array");

    let legacy_json = include_str!("../../../tests/legacy_catalog.json");
    let expected: Vec<Value> =
        serde_json::from_str(legacy_json).expect("Failed to parse legacy_catalog.json");

    assert!(
        entries.len() >= expected.len(),
        "Catalog length mismatch: got {} effects, expected at least {}",
        entries.len(),
        expected.len()
    );

    for exp in &expected {
        let id = exp["id"].as_str().expect("id must be string");
        let name = exp["name"].as_str().expect("name must be string");

        let entry = entries
            .iter()
            .find(|e| e["id"] == id)
            .unwrap_or_else(|| panic!("Missing legacy effect ID: {}", id));

        assert_eq!(
            entry["name"], name,
            "Name mismatch for effect '{}': got '{}', expected '{}'",
            id, entry["name"], name
        );

        let actual_params = entry["parameters"].as_array().expect("parameters array");
        let exp_params = exp["parameters"]
            .as_array()
            .expect("expected parameters array");

        assert_eq!(
            actual_params.len(),
            exp_params.len(),
            "Parameter count mismatch for effect '{}': got {}, expected {}",
            id,
            actual_params.len(),
            exp_params.len()
        );

        for ep in exp_params {
            let pid = ep["id"].as_str().unwrap();
            let ap = actual_params
                .iter()
                .find(|p| p["id"] == pid)
                .unwrap_or_else(|| panic!("Effect '{}' missing parameter '{}'", id, pid));

            assert_eq!(
                ap["label"], ep["label"],
                "Effect '{}' param '{}' label mismatch",
                id, pid
            );
            assert_eq!(
                ap["kind"], ep["kind"],
                "Effect '{}' param '{}' kind mismatch",
                id, pid
            );

            for key in ["min", "max", "step", "default"] {
                let actual_val = ap[key].as_f64().unwrap();
                let expected_val = ep[key].as_f64().unwrap();
                assert!(
                    (actual_val - expected_val).abs() < 1e-4,
                    "Effect '{}' param '{}' key '{}' mismatch: actual={}, expected={}",
                    id,
                    pid,
                    key,
                    actual_val,
                    expected_val
                );
            }
        }
    }
}
