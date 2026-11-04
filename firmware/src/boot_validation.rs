//! Pending-image validation policy; connectivity to external services is irrelevant.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decision {
    Ignore,
    Wait,
    Confirm,
    Fail,
}

#[derive(Clone, Copy, Debug)]
pub struct Observation {
    pub pending: bool,
    pub elapsed_ms: u64,
    pub ready: bool,
    pub failed: bool,
    pub frames: u32,
    pub errors: u32,
    pub progressing: bool,
}

/// Tracks a continuous window with advancing output and no new output errors.
#[derive(Default)]
pub struct HealthWindow {
    previous_frames: u32,
    previous_errors: u32,
    healthy_since_ms: Option<u64>,
}

impl HealthWindow {
    pub fn observe(
        &mut self,
        now_ms: u64,
        ready: bool,
        failed: bool,
        frames: u32,
        errors: u32,
    ) -> u64 {
        let healthy = ready
            && !failed
            && frames != self.previous_frames
            && errors == self.previous_errors;
        self.previous_frames = frames;
        self.previous_errors = errors;
        if !healthy {
            self.healthy_since_ms = None;
            return 0;
        }
        now_ms.saturating_sub(*self.healthy_since_ms.get_or_insert(now_ms))
    }
}

pub fn decide(o: Observation) -> Decision {
    if !o.pending {
        return Decision::Ignore;
    }
    if o.elapsed_ms >= 30_000 {
        return Decision::Fail;
    }
    if o.elapsed_ms >= 5_000
        && o.ready
        && !o.failed
        && o.frames >= 5
        && o.errors == 0
        && o.progressing
    {
        Decision::Confirm
    } else {
        Decision::Wait
    }
}
