//! The one rule for a clipboard write that a program makes with OSC 52.

use super::policy::{ClipboardWrite, PanePolicy};

/// What the pane knows when a program asks.
#[derive(Debug, Clone, Copy)]
pub struct ClipContext {
    pub agent: bool,
    pub owner: bool,
    pub focused: bool,
    pub last_gesture_ms: Option<i64>,
    pub now_ms: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClipRefusal {
    Write,
    Read,
    TooLarge,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClipDecision {
    /// Copy this text.
    Allow(String),
    /// Drop the request and tell the operator.
    Refuse(ClipRefusal),
    /// Drop the request without a word: the payload is not text.
    Drop,
}

/// Decides one OSC 52 payload, `<selection>;<data>`. The selection field is
/// ignored. The order is: shell panes never, a read never, then the policy,
/// then the payload (bad base64 and bad UTF-8 are dropped), then the size,
/// then the focus and the operator's gesture.
pub fn decide(policy: &PanePolicy, ctx: &ClipContext, payload: &str) -> ClipDecision {
    let data = payload.split_once(';').map_or(payload, |(_, data)| data);
    if !ctx.agent {
        return ClipDecision::Refuse(if data == "?" {
            ClipRefusal::Read
        } else {
            ClipRefusal::Write
        });
    }
    if data == "?" {
        return ClipDecision::Refuse(ClipRefusal::Read);
    }
    if policy.clipboard_write == ClipboardWrite::Deny {
        return ClipDecision::Refuse(ClipRefusal::Write);
    }
    let Some(bytes) = decode_base64(data) else {
        return ClipDecision::Drop;
    };
    let Ok(text) = String::from_utf8(bytes) else {
        return ClipDecision::Drop;
    };
    if text.is_empty() {
        return ClipDecision::Drop;
    }
    if text.len() > policy.max_decoded_bytes {
        return ClipDecision::Refuse(ClipRefusal::TooLarge);
    }
    let in_window = ctx
        .last_gesture_ms
        .is_some_and(|at| ctx.now_ms.saturating_sub(at) <= i64::from(policy.gesture_window_ms));
    if !ctx.owner || !ctx.focused || !in_window {
        return ClipDecision::Refuse(ClipRefusal::Write);
    }
    ClipDecision::Allow(text)
}

fn decode_base64(data: &str) -> Option<Vec<u8>> {
    let trimmed = data.trim_end_matches('=');
    if trimmed.len() % 4 == 1 || data.len() - trimmed.len() > 2 {
        return None;
    }
    let mut out = Vec::with_capacity(trimmed.len() / 4 * 3 + 2);
    let (mut acc, mut bits) = (0u32, 0u32);
    for c in trimmed.bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => return None,
        };
        acc = acc << 6 | u32::from(v);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits & 0xff) as u8);
        }
    }
    Some(out)
}
