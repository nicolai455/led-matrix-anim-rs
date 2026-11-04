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
fn transient_output_error_requires_a_fresh_healthy_window() {
    use matrix_firmware::boot_validation::HealthWindow;
    let mut window = HealthWindow::default();
    assert_eq!(window.observe(1000, true, false, 5, 0), 0);
    assert_eq!(window.observe(2000, true, false, 10, 1), 0);
    assert_eq!(window.observe(3000, true, false, 15, 1), 0);
    assert_eq!(window.observe(7999, true, false, 20, 1), 4999);
    assert_eq!(window.observe(8000, true, false, 25, 1), 5000);
}

#[test]
fn unhealthy_samples_restart_the_window() {
    use matrix_firmware::boot_validation::HealthWindow;
    let mut w = HealthWindow::default();
    assert_eq!(w.observe(1000, true, false, 1, 0), 0);
    assert_eq!(w.observe(5000, true, false, 5, 0), 4000);
    assert_eq!(w.observe(6000, true, false, 5, 0), 0);
    assert_eq!(w.observe(7000, true, false, 6, 0), 0);
    assert_eq!(w.observe(8000, false, false, 7, 0), 0);
    assert_eq!(w.observe(9000, true, false, 8, 0), 0);
    assert_eq!(w.observe(10000, true, true, 9, 0), 0);
    assert_eq!(w.observe(11000, true, false, 10, 0), 0);
    assert_eq!(w.observe(16000, true, false, 15, 0), 5000);
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
