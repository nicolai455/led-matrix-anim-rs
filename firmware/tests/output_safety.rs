use matrix_control::LedOutput;
use matrix_core::{Frame, PanelConfig};
use matrix_firmware::output::{OutputQuiesce, OutputSettings, RmtLedOutput};
use std::time::Duration;

#[test]
fn output_driver_does_not_embed_frame_buffers_on_task_stacks() {
    assert!(std::mem::size_of::<RmtLedOutput>() < 1024);
}

#[test]
fn unacknowledged_blackout_times_out_instead_of_allowing_ota() {
    let control = OutputQuiesce::default();
    assert!(control.pause(Duration::from_millis(2)).is_err());
    control.resume();
    assert!(control.pause(Duration::ZERO).is_err());
}

#[test]
fn host_cannot_claim_physical_transmission_or_blackout() {
    let mut output = RmtLedOutput::new(OutputSettings::default(), PanelConfig::default());
    assert!(output.blank().is_err());
    assert!(output.show(&Frame::default(), 0).is_err());
}
