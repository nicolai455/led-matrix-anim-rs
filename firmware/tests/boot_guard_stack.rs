// The guard is ESP-IDF-only, so check its thread configuration on the host too.
#[test]
fn ota_validation_uses_a_conservative_explicit_stack_budget() {
    let source = include_str!("../src/boot_guard.rs");
    let builder = source.split("std::thread::Builder::new()").nth(1)
        .expect("boot validation thread builder");
    let config = builder.split(".spawn(").next().expect("thread spawn");
    assert!(config.contains(".name(\"boot-validation\".into())"));
    assert!(config.contains(".stack_size(6144)"));
}
