use std::cell::{Cell, RefCell};

use crate::ui::text::width;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reason {
    pub text: String,
    pub short: Option<String>,
}

impl Reason {
    pub const TEXT_MAX: usize = 60;

    pub const SHORT_MAX: usize = 20;

    pub fn new(text: &str) -> Self {
        Self {
            text: text.to_owned(),
            short: None,
        }
    }

    pub fn short(mut self, short: &str) -> Self {
        self.short = Some(short.to_owned());
        self
    }

    pub fn within_limits(&self) -> bool {
        width(&self.text) <= Self::TEXT_MAX
            && self
                .short
                .as_deref()
                .is_none_or(|s| width(s) <= Self::SHORT_MAX)
    }

    pub fn text_fits(&self, room: usize) -> bool {
        width(&self.text) <= room
    }

    pub fn short_fits(&self, room: usize) -> bool {
        self.short.as_deref().is_some_and(|s| width(s) <= room)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum Control {
    #[default]
    Live,

    Fixed(Reason),
}

impl Control {
    pub fn fixed(reason: Reason) -> Self {
        Self::Fixed(reason)
    }

    pub fn is_fixed(&self) -> bool {
        matches!(self, Self::Fixed(_))
    }

    pub fn reason(&self) -> Option<&Reason> {
        match self {
            Self::Live => None,
            Self::Fixed(reason) => Some(reason),
        }
    }

    pub fn refuse(&self) -> bool {
        match self {
            Self::Live => false,
            Self::Fixed(reason) => {
                raise(reason);
                true
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReasonId(u16);

impl ReasonId {
    pub fn text(self) -> String {
        TABLE.with_borrow(|table| table[usize::from(self.0)].text.clone())
    }
}

thread_local! {
    static TABLE: RefCell<Vec<Reason>> = const { RefCell::new(Vec::new()) };
    static SLOT: Cell<Option<ReasonId>> = const { Cell::new(None) };
}

fn intern(reason: &Reason) -> ReasonId {
    TABLE.with_borrow_mut(|table| {
        let at = table.iter().position(|r| r == reason).unwrap_or_else(|| {
            table.push(reason.clone());
            table.len() - 1
        });
        ReasonId(u16::try_from(at).expect("fewer than 65536 distinct reasons"))
    })
}

pub fn raise(reason: &Reason) {
    SLOT.set(Some(intern(reason)));
}

pub fn take_refusal() -> Option<ReasonId> {
    SLOT.take()
}

pub fn clear_refusal() {
    SLOT.set(None);
}
