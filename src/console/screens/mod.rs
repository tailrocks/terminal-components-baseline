pub mod accounts;
pub mod capsule;
pub mod cockpit;
pub mod config;
pub mod editor;
pub mod inspect;
pub mod manager;
pub mod modals;
pub mod prelude;
pub mod rowmenu;
pub mod settings;
pub mod usage;

use crate::core::event::Outcome;
use crate::widgets::option_list::Item;
use crate::widgets::segments::Segment;

use crate::console::domain::account::AccountId;
use crate::console::domain::agent::Agent;
use crate::console::domain::instance::InstanceId;
use crate::console::domain::onepassword::OpReference;
use crate::console::domain::workspace::{Workspace, WorkspaceId};
use crate::console::launch::LaunchPlan;
use crate::console::world::{Msg, World};
use crate::ui::layer::Layer;
pub use crate::ui::layer::LayerTag;
use crate::ui::layers::LayerResult;
use crate::widgets::menu::MenuAction;
use crate::widgets::picker::PickerAction;
use modals::{BrowserResult, FormAction, FormDialog, FormValues, InfoResult};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModalResult {
    Dialog {
        action: Option<usize>,
        text: Option<String>,
    },
    Picked(usize),
    PickedAlt(usize),

    Scope,

    PickerEvent(PickerAction),
    Cancelled,
    Browser(BrowserResult),
    Choice(Option<usize>),
    Form(Option<FormValues>),

    FormAction(String, FormValues),

    FormEvent(FormAction),
    Op(Option<OpReference>),
    Info(InfoResult),

    Help,
    Custom(String),
}

impl From<LayerResult> for ModalResult {
    fn from(r: LayerResult) -> Self {
        match r {
            LayerResult::Dialog { action, text } => ModalResult::Dialog { action, text },
            LayerResult::Picker(PickerAction::Chosen(i))
            | LayerResult::Menu(MenuAction::Chosen(i)) => ModalResult::Picked(i),
            LayerResult::Picker(PickerAction::ChosenAlt(i)) => ModalResult::PickedAlt(i),
            LayerResult::Picker(PickerAction::NextScope | PickerAction::PrevScope) => {
                ModalResult::Scope
            }
            LayerResult::Picker(PickerAction::Cancelled) | LayerResult::Menu(_) => {
                ModalResult::Cancelled
            }
            LayerResult::Picker(ev) => ModalResult::PickerEvent(ev),
            LayerResult::Form(FormAction::Cancel) => ModalResult::Form(None),
            LayerResult::Form(ev) => ModalResult::FormEvent(ev),
            LayerResult::Choice(c) => ModalResult::Choice(c),
            LayerResult::Info(r) => ModalResult::Info(r),
            LayerResult::Help => ModalResult::Help,
            LayerResult::File(r) => ModalResult::Browser(r),
        }
    }
}

pub type AppLayer = dyn Layer<ModalResult, World>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Go {
    Manager,
    Settings,
    Accounts {
        select: Option<AccountId>,
    },
    Usage {
        select: Option<AccountId>,
    },

    Editor {
        workspace: Option<WorkspaceId>,
        pending: Option<Box<Workspace>>,
    },
    Prelude,
    Launch {
        workspace: Option<WorkspaceId>,
        role: String,
        agent: Agent,
        account: Option<AccountId>,
        plan: LaunchPlan,
    },
    Attach {
        instance: InstanceId,
        pane: Option<u64>,
    },
    NewSession {
        instance: InstanceId,
        agent: Option<Agent>,
        account: Option<AccountId>,
    },

    Detach,

    InstanceEnded {
        instance: InstanceId,
        purge: bool,
    },

    LaunchFailedAck {
        instance: Option<InstanceId>,
    },
    Quit,
}

pub type Request = crate::shell::Request<ModalResult, World, Go>;

pub type Cx<'a> = crate::shell::Cx<'a, ModalResult, World, Go>;

pub struct Jackin;

impl crate::shell::PageTypes for Jackin {
    type State = World;
    type Msg = Msg;
    type Result = ModalResult;
    type LayerState = World;
    type Go = Go;
}

pub trait Screen: crate::shell::Page<Jackin> {
    fn picker_items(&mut self, _tag: &LayerTag, _query: &str, _w: &World) -> Option<Vec<Item>> {
        None
    }

    fn form_changed(&mut self, _tag: &LayerTag, _form: &mut FormDialog, _w: &World) {}

    fn strip_right(&self, _w: &World) -> Vec<Segment> {
        vec![]
    }

    fn layer_open(&self) -> bool {
        false
    }

    fn request_leave(&mut self, _w: &mut World, cx: &mut Cx) -> Outcome {
        cx.go(Go::Manager);
        Outcome::Changed
    }
}

pub fn health_spans(
    s: &crate::console::domain::usage::OverallSummary,
) -> Vec<crate::widgets::viewport::Span> {
    use crate::console::domain::usage::HealthWord;
    use crate::theme::Tone;
    use crate::widgets::viewport::Span;
    let word = Span::new(
        s.health.label(),
        match s.health {
            HealthWord::Blocked | HealthWord::Degraded => Tone::Error,
            HealthWord::Attention => Tone::Warning,
            _ => Tone::Normal,
        },
    );
    let word = if matches!(s.health, HealthWord::Blocked | HealthWord::Degraded) {
        word.bold()
    } else {
        word
    };
    let mut v = vec![word];
    for (text, tone) in s.issue_parts() {
        v.push(Span::new(" · ", Tone::Muted));
        v.push(Span::new(text, tone));
    }
    v
}
