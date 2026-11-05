use matrix_firmware::ota::RestartSignal;
use std::time::{Duration, Instant};

#[test]
fn restart_signal_waits_for_response_grace_period() {
    let signal = RestartSignal::default();
    let now = Instant::now();
    assert!(!signal.take_due(now));
    signal.request(now, Duration::from_secs(1));
    assert!(!signal.take_due(now + Duration::from_millis(999)));
    assert!(signal.take_due(now + Duration::from_secs(1)));
    assert!(!signal.take_due(now + Duration::from_secs(2)));
}

#[test]
fn repeated_requests_do_not_postpone_restart() {
    let signal = RestartSignal::default();
    let now = Instant::now();
    signal.request(now, Duration::from_secs(1));
    signal.request(now + Duration::from_millis(500), Duration::from_secs(1));
    assert!(signal.take_due(now + Duration::from_secs(1)));
}
