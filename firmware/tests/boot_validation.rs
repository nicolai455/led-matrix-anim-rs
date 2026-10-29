use matrix_firmware::boot_validation::{decide, Decision, Observation};

fn observation() -> Observation {
    Observation {
        pending: true,
        elapsed_ms: 5000,
        ready: true,
        failed: false,
        frames: 5,
        errors: 0,
        progressing: true,
    }
}

#[test]
fn only_pending_healthy_sustained_progress_is_confirmed() {
    let o = observation();
    assert_eq!(decide(o), Decision::Confirm);
    assert_eq!(
        decide(Observation {
            pending: false,
            ..o
        }),
        Decision::Ignore
    );
    assert_eq!(
        decide(Observation {
            elapsed_ms: 4999,
            ..o
        }),
        Decision::Wait
    );
    assert_eq!(
        decide(Observation {
            progressing: false,
            ..o
        }),
        Decision::Wait
    );
    assert_eq!(decide(Observation { errors: 1, ..o }), Decision::Wait);
    assert_eq!(decide(Observation { failed: true, ..o }), Decision::Wait);
    assert_eq!(decide(Observation { ready: false, ..o }), Decision::Wait);
    assert_eq!(decide(Observation { frames: 1, ..o }), Decision::Wait);
}

#[test]
fn deadline_takes_precedence_over_late_health() {
    let o = observation();
    assert_eq!(
        decide(Observation {
            elapsed_ms: 30000,
            ..o
        }),
        Decision::Fail
    );
    assert_eq!(
        decide(Observation {
            elapsed_ms: 30000,
            ready: false,
            ..o
        }),
        Decision::Fail
    );
    assert_eq!(
        decide(Observation {
            pending: false,
            elapsed_ms: 30000,
            ..o
        }),
        Decision::Ignore
    );
}
