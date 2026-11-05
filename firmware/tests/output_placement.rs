//! Exercise target-only placement code on the host with a recording allocator.
//! This does not test ESP-IDF's allocator or hardware DMA behaviour.
use std::{fs, process::Command};

const OUTPUT: &str = include_str!("../src/output.rs");

#[test]
fn backend_and_allocator_capabilities_are_wired_together() {
    // Compile the actual target-only constructor and backend expression against a
    // recording allocator, rather than reimplementing their decisions in a test.
    let helper = OUTPUT
        .split("fn prefer_psram<T>(")
        .nth(1)
        .unwrap()
        .split("\n/// RMT's")
        .next()
        .unwrap();
    let constructor = OUTPUT
        .split("impl OutputBuffer {")
        .nth(1)
        .unwrap()
        .split("\n}\n")
        .next()
        .unwrap();
    let backend = OUTPUT
        .split("let buffer = OutputBuffer::new(")
        .nth(1)
        .unwrap()
        .split(");")
        .next()
        .unwrap();
    // The expression must depend on the actual selected backend, not merely a
    // user preference or whether LCD initialization happened to succeed.
    assert_eq!(backend.trim(), "cfg!(esp32s3) && settings.parallel_output");
    assert!(OUTPUT.contains("let use_rmt = !cfg!(esp32s3) || !settings.parallel_output;"));

    let harness = format!(
        r#"
use std::sync::Mutex;
const TOTAL_BYTES: usize = 3072;
static CALLS: Mutex<Vec<u32>> = Mutex::new(Vec::new());
static FAIL_EXTERNAL: Mutex<bool> = Mutex::new(false);
mod esp_idf_sys {{
    pub const MALLOC_CAP_SPIRAM: u32 = 1;
    pub const MALLOC_CAP_INTERNAL: u32 = 2;
    pub const MALLOC_CAP_8BIT: u32 = 4;
    pub unsafe fn heap_caps_calloc(n: usize, size: usize, caps: u32) -> *mut u8 {{
        assert_eq!((n, size), (super::TOTAL_BYTES, 1));
        super::CALLS.lock().unwrap().push(caps);
        if caps & MALLOC_CAP_SPIRAM != 0 && *super::FAIL_EXTERNAL.lock().unwrap() {{
            std::ptr::null_mut()
        }} else {{
            std::ptr::NonNull::<u8>::dangling().as_ptr()
        }}
    }}
}}
fn prefer_psram<T>({helper}
struct OutputBuffer(std::ptr::NonNull<u8>);
impl OutputBuffer {{{constructor}
}}
struct Settings {{ parallel_output: bool }}
fn run(parallel_output: bool, fail_external: bool, expected: &[u32]) {{
    CALLS.lock().unwrap().clear();
    *FAIL_EXTERNAL.lock().unwrap() = fail_external;
    let settings = Settings {{ parallel_output }};
    let buffer = OutputBuffer::new({backend});
    assert!(!buffer.0.as_ptr().is_null());
    assert_eq!(*CALLS.lock().unwrap(), expected);
}}
fn main() {{
    // Non-S3 is always RMT; S3 uses LCD only in parallel mode.
    if cfg!(esp32s3) {{
        run(true, false, &[5]);
        run(true, true, &[5, 6]);
        run(false, false, &[6]);
        run(false, true, &[6]);
    }} else {{
        run(true, false, &[6]);
        run(false, false, &[6]);
    }}
}}
"#
    );
    let dir = std::env::temp_dir().join(format!("output-placement-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let source = dir.join("placement.rs");
    fs::write(&source, harness).unwrap();
    for s3 in [false, true] {
        let binary = dir.join(if s3 { "s3" } else { "other" });
        let mut rustc = Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()));
        rustc
            .arg("--edition=2021")
            .arg(&source)
            .arg("-o")
            .arg(&binary);
        if s3 {
            rustc.args(["--cfg", "esp32s3"]);
        }
        let output = rustc.output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let output = Command::new(binary).output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    fs::remove_dir_all(dir).unwrap();
}
