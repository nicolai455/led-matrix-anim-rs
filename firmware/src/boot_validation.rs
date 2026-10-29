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
