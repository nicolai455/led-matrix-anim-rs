use matrix_firmware::recovery::{Journal, Watchdog};

#[test]
fn repeated_incomplete_boots_latch_recovery_until_explicit_retry() {
    let mut j = Journal::default();
    for _ in 0..4 {
        j.begin();
    }
    assert!(j.safe_mode);
    j.healthy();
    assert!(j.safe_mode);
    j.retry();
    assert!(!j.safe_mode);
    j.begin();
    assert_eq!(j.failures, 0);
}

#[test]
fn planned_restart_does_not_count_as_failure() {
    let mut j = Journal::default();
    j.begin();
    j.planned();
    j.begin();
    assert_eq!(j.failures, 0);
}

#[test]
fn inactive_recovery_allows_ota_without_claiming_blackout_or_hardware_output() {
    use matrix_control::LedOutput;
    let q = matrix_firmware::output::OutputQuiesce::inactive();
    let mut sink = matrix_firmware::recovery::RecoveryOutput::Inactive(q.clone());
    assert!(q.pause(std::time::Duration::ZERO).is_ok());
    q.resume();
    assert!(q.pause(std::time::Duration::ZERO).is_ok());
    assert!(sink.show(&matrix_core::Frame::default(), 0).is_err());
}

#[test]
fn repaired_image_gets_one_trial_without_losing_recovery_latch() {
    let mut j = Journal::default();
    j.safe_mode = true;
    j.failures = 3;
    j.arm_trial();
    j.begin();
    assert!(!j.safe_mode);
    j.begin();
    assert!(j.safe_mode);
}

#[test]
fn trial_for_new_partition_cannot_bypass_old_image_recovery() {
    let mut j = Journal::default();
    j.safe_mode = true;
    j.failures = 3;
    j.arm_trial();
    j.trial_partition = Some(0x420000);
    j.begin_partition(Some(0x20000));
    assert!(j.safe_mode);
    assert!(!j.trial);
}

#[test]
fn watchdog_requires_render_and_output_but_tolerates_intentional_pause() {
    let mut w = Watchdog::new(0, 0, 0);
    assert!(!w.stalled(1000, 1, 1, false));
    assert!(w.stalled(7001, 2, 1, false));
    assert!(!w.stalled(8000, 2, 1, true));
    assert!(!w.stalled(9000, 3, 2, false));
}
