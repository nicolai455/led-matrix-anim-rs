//! Deterministic policy sequences; these do not exercise the ESP-IDF guard,
//! rollback calls, thread stack usage, or physical output.
use matrix_firmware::boot_validation::{decide, Decision, HealthWindow, Observation};

#[derive(Default)]
struct PolicyDriver {
    window: HealthWindow,
    previous_frames: u32,
    previous_errors: u32,
}

impl PolicyDriver {
    // Mirrors the guard's policy inputs, not its platform or recovery actions.
    fn sample(
        &mut self,
        now: u64,
        ready: bool,
        failed: bool,
        frames: u32,
        errors: u32,
    ) -> Decision {
        let progressing = frames != self.previous_frames;
        let new_errors = errors.wrapping_sub(self.previous_errors);
        self.previous_frames = frames;
        self.previous_errors = errors;
        let healthy_ms = self.window.observe(now, ready, failed, frames, errors);
        if now >= 30_000 {
            Decision::Fail
        } else {
            decide(Observation {
                pending: true,
                elapsed_ms: healthy_ms,
                ready,
                failed,
                frames,
                errors: new_errors,
                progressing,
            })
        }
    }
}

#[test]
fn first_healthy_sample_starts_window_not_confirmation() {
    let mut p = PolicyDriver::default();
    for second in 1..=5 {
        assert_eq!(
            p.sample(second * 1000, true, false, second as u32, 0),
            Decision::Wait
        );
    }
    assert_eq!(p.sample(6000, true, false, 6, 0), Decision::Confirm);
}

#[test]
fn output_error_or_stall_requires_five_new_healthy_seconds() {
    for error in [false, true] {
        let mut p = PolicyDriver::default();
        for second in 1..=5 {
            assert_eq!(
                p.sample(second * 1000, true, false, second as u32, 0),
                Decision::Wait
            );
        }
        let count = u32::from(error);
        let frames = if error { 6 } else { 5 };
        assert_eq!(p.sample(6000, true, false, frames, count), Decision::Wait);
        for second in 7..=11 {
            assert_eq!(
                p.sample(
                    second * 1000,
                    true,
                    false,
                    frames + second as u32 - 6,
                    count
                ),
                Decision::Wait
            );
        }
        assert_eq!(
            p.sample(12000, true, false, frames + 6, count),
            Decision::Confirm
        );
    }
}

#[test]
fn real_deadline_wins_when_new_healthy_window_reaches_five_seconds() {
    let mut p = PolicyDriver::default();
    // No ready signal until 25 seconds: the first healthy sample starts the window.
    assert_eq!(p.sample(24_000, false, false, 24, 0), Decision::Wait);
    for second in 25..30 {
        assert_eq!(
            p.sample(second * 1000, true, false, second as u32, 0),
            Decision::Wait
        );
    }
    assert_eq!(p.sample(30_000, true, false, 30, 0), Decision::Fail);
}

#[test]
fn failed_latch_or_missing_progress_cannot_confirm_before_deadline() {
    for failed in [false, true] {
        let mut p = PolicyDriver::default();
        for second in 1..30 {
            let frames = if failed { second as u32 } else { 0 };
            assert_eq!(
                p.sample(second * 1000, true, failed, frames, 0),
                Decision::Wait
            );
        }
        assert_eq!(p.sample(30_000, true, failed, 30, 0), Decision::Fail);
    }
}
