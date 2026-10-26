fn main() {
    println!("cargo::rustc-check-cfg=cfg(esp32s3)");
    println!("cargo:rerun-if-changed=partitions.csv");
    println!("cargo:rerun-if-changed=partitions_16MB.csv");
    println!("cargo:rerun-if-env-changed=MATRIX_LINK_MAP");

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("espidf") {
        embuild::espidf::sysenv::output();
        // ESP-IDF also emits --Map; the application map must be last.
        if let Ok(path) = std::env::var("MATRIX_LINK_MAP") {
            println!("cargo:rustc-link-arg=-Wl,-Map={path}");
        }

        let target = std::env::var("TARGET").unwrap_or_default();
        let is_s3 = target.contains("esp32s3") || std::env::var("CARGO_FEATURE_ESP32S3").is_ok();
        if is_s3 {
            println!("cargo:rustc-cfg=esp32s3");
        }

        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".into());
        let manifest_path = std::path::Path::new(&manifest_dir);

        if let Ok(out_dir) = std::env::var("OUT_DIR") {
            let out_path = std::path::Path::new(&out_dir);
            let _ = std::fs::copy(
                manifest_path.join("partitions.csv"),
                out_path.join("partitions.csv"),
            );
            let _ = std::fs::copy(
                manifest_path.join("partitions_16MB.csv"),
                out_path.join("partitions_16MB.csv"),
            );
        }
    }
}
