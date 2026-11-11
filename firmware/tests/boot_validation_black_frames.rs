use matrix_firmware::boot_validation::{decide, Decision, HealthWindow, Observation};

#[test]
fn pending_image_can_confirm_after_ready_off_output_is_advancing() {
    let mut window = HealthWindow::default();
    let mut decision = Decision::Wait;
    for second in 0..=6 {
        let now_ms = second * 1_000;
        let frames = second as u32 + 1;
        let healthy_ms = window.observe(now_ms, true, false, frames, 0);
        decision = decide(Observation {
            pending: true,
            elapsed_ms: healthy_ms,
            ready: true,
            failed: false,
            frames,
            errors: 0,
            progressing: true,
        });
    }
    assert_eq!(decision, Decision::Confirm);
}

#[test]
fn boot_dark_pending_trial_may_confirm_on_successful_black_frames() {
    let mut window = HealthWindow::default();
    let mut confirmed = false;
    for second in 0..=6 {
        let now_ms = second * 1_000;
        let frames = second as u32 + 1;
        let healthy_ms = window.observe(now_ms, true, false, frames, 0);
        let decision = decide(Observation {
            pending: true,
            elapsed_ms: healthy_ms,
            ready: true,
            failed: false,
            frames,
            errors: 0,
            progressing: true,
        });
        confirmed |= decision == Decision::Confirm;
    }
    assert!(confirmed, "OFF/black successful output must not block a healthy OTA trial");
}
