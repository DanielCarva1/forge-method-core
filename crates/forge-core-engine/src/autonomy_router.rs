//! Autonomy router — lane selection over autonomy_policy + verification_goal. Stub, fleshed out by Wave 4 worker.

/// Lane kind — stub.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaneKind {
    Fast,
    Rigorous,
}

/// Lane decision — stub.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaneDecision {
    pub lane: LaneKind,
    pub reason: LaneRouteReason,
}

/// Lane route reason — stub.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaneRouteReason {
    Placeholder,
}

/// Route a change to a lane — stub, fleshed out by worker.
pub fn route_lane() -> LaneDecision {
    LaneDecision {
        lane: LaneKind::Fast,
        reason: LaneRouteReason::Placeholder,
    }
}
