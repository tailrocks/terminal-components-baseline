/// What the console lets a program do through a pane, apart from drawing.
///
/// The product default lets the focused agent pane write the clipboard right
/// after the operator typed or clicked in it; a host that wants none sets
/// `Deny`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PanePolicy {
    pub clipboard_write: ClipboardWrite,
    /// How long after the operator's last key or click in a pane a clipboard
    /// write is still taken as the operator's.
    pub gesture_window_ms: u32,
    /// The most bytes a clipboard write may hold once decoded.
    pub max_decoded_bytes: usize,
    pub paste_max_bytes: usize,
    /// A refused clipboard write shows its status at most this often for one
    /// pane.
    pub refusal_status_every_ms: u32,
    /// The most output bytes one pane feeds into its grid in one tick.
    pub feed_budget_bytes: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClipboardWrite {
    Deny,
    AgentOnGesture,
}

impl Default for PanePolicy {
    fn default() -> Self {
        Self {
            clipboard_write: ClipboardWrite::AgentOnGesture,
            gesture_window_ms: 5_000,
            max_decoded_bytes: 100_000,
            paste_max_bytes: 64 * 1024,
            refusal_status_every_ms: 10_000,
            feed_budget_bytes: 64 * 1024,
        }
    }
}

/// How long the replay window of an attach stays open. The live backend fills
/// it from the repaint nudge of its transport.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PaneTiming {
    /// The window closes this long after the attach at the latest.
    pub replay_cap_ms: i64,
    /// The window cannot close before this long after the attach.
    pub nudge_ms: i64,
    /// After the nudge it closes once the output has been quiet this long.
    pub quiet_ms: i64,
}

impl Default for PaneTiming {
    fn default() -> Self {
        Self {
            replay_cap_ms: 800,
            nudge_ms: 600,
            quiet_ms: 150,
        }
    }
}
