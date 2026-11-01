use crate::theme::StatusKind;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DoctorSurface {
    pub noun: String,
    pub state: DoctorState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DoctorState {
    NeverRun,
    Running { previous: Option<DoctorReport> },
    Done(DoctorReport),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DoctorReport {
    pub checks: Vec<DoctorCheck>,
    pub at_secs: i64,
    pub gateway: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DoctorCheck {
    pub label: String,
    pub status: StatusKind,
    pub summary: String,
    pub found: Option<String>,
    pub expected: Option<String>,
    pub fix: Option<String>,
}

pub const STALE_AFTER_SECS: i64 = 60;

impl DoctorReport {
    pub fn failing(&self) -> usize {
        self.count(StatusKind::Fail)
    }

    pub fn warnings(&self) -> usize {
        self.count(StatusKind::Warn)
    }

    fn count(&self, kind: StatusKind) -> usize {
        self.checks.iter().filter(|c| c.status == kind).count()
    }

    pub fn stale(&self, now_secs: i64) -> bool {
        now_secs - self.at_secs > STALE_AFTER_SECS
    }
}

impl DoctorSurface {
    pub fn report(&self) -> Option<&DoctorReport> {
        match &self.state {
            DoctorState::NeverRun => None,
            DoctorState::Running { previous } => previous.as_ref(),
            DoctorState::Done(r) => Some(r),
        }
    }

    pub fn running(&self) -> bool {
        matches!(self.state, DoctorState::Running { .. })
    }
}
