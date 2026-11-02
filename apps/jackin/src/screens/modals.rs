use junie_tui::core::event::{Key, Outcome};

use junie_tui::core::hit::Axis;
use junie_tui::core::id::WidgetId;
use junie_tui::core::intent::Phase;
use junie_tui::core::pointer::{MouseTarget, PointerEvent};
use junie_tui::ui::ctx::RenderCtx;
use junie_tui::ui::layer::{DismissReason, Layer, LayerCx, LayerOut, LayerSpec};
use junie_tui::widgets::keyhint::{Hint, hint};
use junie_tui::widgets::option_list::Item;
use junie_tui::widgets::picker::{Picker, PickerAction, PickerStatus};
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::Rect;

use crate::backend::{FieldKind, OnePassword, OpError};
use crate::domain::onepassword::OpReference;
use crate::world::World;

use super::ModalResult;
use crate::domain::docker::DockerRiskText;
use junie_tui::core::scroll::Scrollable;
use junie_tui::theme::StatusKind;
use junie_tui::ui::text::plural;
use junie_tui::widgets::button::Button;
use junie_tui::widgets::dialog::Dialog;
use junie_tui::widgets::props::Prop;

pub fn unsaved_dialog(id: WidgetId, changes: usize) -> Dialog {
    Dialog::confirm(
        id,
        "Unsaved changes",
        &format!(
            "Save changes before leaving? {} would be lost.",
            plural(changes.max(1), "change", "changes")
        ),
        "Save",
    )
    .actions(
        vec![
            Button::subtle(id.sub("exit_cancel"), "Cancel"),
            Button::danger(id.sub("exit_discard"), "Discard"),
            Button::primary(id.sub("exit_save"), "Save"),
        ],
        Some(0),
    )
}

pub fn docker_row_hints() -> [Hint; 2] {
    [hint("Space", "Toggle"), hint("i", "Risks")]
}

pub fn docker_risk_dialog(
    id: WidgetId,
    subject: &str,
    risks: &[DockerRiskText],
    confirm: bool,
) -> Dialog {
    let facts: Vec<Prop> = risks
        .iter()
        .map(|r| Prop::new(&r.label, r.text.clone()).wrap())
        .collect();
    if confirm {
        return Dialog::facts(
            id,
            &format!("Turn on Docker for {subject}?"),
            facts,
            vec![],
            None,
            Button::primary(id.sub("turn_on"), "Turn on"),
        )
        .title_status(StatusKind::Warn)
        .width(92);
    }
    let close = Button::primary(id.sub("close"), "Close");
    let focus = close.id;
    let mut d = Dialog::facts(
        id,
        &format!("Docker risks · {subject}"),
        facts,
        vec![],
        None,
        close.clone(),
    )
    .title_status(StatusKind::Warn)
    .width(92)
    .actions(vec![close], Some(0));
    d.initial_focus = focus;
    d
}

use junie_tui::widgets::file_browser::{Choose, FileCmd, FileEntry, MemorySource, UrlEntry};
pub use junie_tui::widgets::file_browser::{FileBrowser, FileBrowserResult as BrowserResult};

pub fn file_browser(
    id: WidgetId,
    title: &str,
    cwd: &str,
    with_readonly: bool,
    dirs_only: bool,
    w: &World,
) -> FileBrowser {
    let source = MemorySource::new(
        w.fs.iter()
            .map(|e| {
                let meta = match (&e.git, e.meta.as_str()) {
                    (Some(b), _) => format!("git · {b}"),
                    (None, m) => m.to_owned(),
                };
                let entry = if e.dir {
                    FileEntry::dir(&e.path)
                } else {
                    FileEntry::file(&e.path)
                };
                entry.meta(&meta)
            })
            .collect(),
    )
    .home(&w.home);
    let repos: Vec<String> = w.github.iter().map(|r| r.full_name.clone()).collect();
    let url = UrlEntry::new("Git URL…", "Git URL")
        .back("Browse folders")
        .placeholder("github.com/org/repo")
        .help(&[
            "Enter a GitHub repository URL. The repository is cloned into the",
            "Construct at launch; the branch is chosen in the mount row.",
        ])
        .empty_error("Enter a repository URL")
        .validate(move |url| {
            if repos.iter().any(|r| url.contains(r.as_str())) {
                Ok(())
            } else {
                Err(format!("Could not resolve {url}: repository not found"))
            }
        });
    let b = FileBrowser::new(id, title, source, cwd)
        .choose(if dirs_only { Choose::Dirs } else { Choose::Any })
        .keys(FileCmd::vim_keymap())
        .show_hidden(true)
        .hidden_toggle(false)
        .url_entry(url);
    if with_readonly {
        b.checkbox("Mount read-only", false)
    } else {
        b
    }
}

pub use junie_tui::widgets::choice_dialog::ChoiceDialog;
pub use junie_tui::widgets::form_dialog::{
    FieldValue, FormAction, FormDialog, FormField, FormFieldKind as FieldKindW, FormValues,
};
pub use junie_tui::widgets::info_dialog::{InfoDialog, InfoResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpStep {
    Account,
    Vault,
    Item,
    Field,
}

pub struct OpFlow {
    pub dismiss: junie_tui::ui::popup::Dismiss,
    pub step: OpStep,
    pub account: Option<String>,
    pub vault: Option<(String, String)>,
    pub item: Option<(String, String)>,
    pub picker: Picker,
    pub result: Option<Option<OpReference>>,

    pub dismissed: bool,
    pub loading_until: Option<i64>,
    pub last_error: Option<OpError>,

    pub concealed_only: bool,
}

impl OpFlow {
    pub fn new(id: WidgetId, op: &dyn OnePassword, now_ms: i64) -> Self {
        let mut f = Self {
            dismiss: junie_tui::ui::popup::Dismiss::LIGHT,
            step: OpStep::Account,
            account: None,
            vault: None,
            item: None,
            picker: Picker::new(id.sub("picker"), "1Password"),
            result: None,
            dismissed: false,
            loading_until: None,
            last_error: None,
            concealed_only: true,
        };
        f.picker.list.width = (64, 64);

        let avail: Vec<&crate::backend::OpAccount> = op
            .accounts()
            .iter()
            .filter(|a| a.state == crate::backend::OpAccountState::Available)
            .collect();
        if op.accounts().len() == 1 || (avail.len() == 1 && op.accounts().len() == 1) {
            f.account = Some(op.accounts()[0].id.clone());
            f.step = OpStep::Vault;
        }
        f.begin_load(now_ms, op);
        f
    }

    fn crumb(&self) -> String {
        let mut parts = vec!["1Password".to_owned()];
        if let Some(a) = &self.account {
            parts.push(a.split('.').next().unwrap_or(a).to_owned());
        }
        if let Some((_, v)) = &self.vault {
            parts.push(v.clone());
        }
        if let Some((_, i)) = &self.item {
            parts.push(i.clone());
        }
        parts.join(" › ")
    }

    fn begin_load(&mut self, now_ms: i64, op: &dyn OnePassword) {
        self.picker.set_query("");
        self.picker.title = match self.step {
            OpStep::Account => "Choose 1Password account".into(),
            OpStep::Vault => "Choose vault".into(),
            OpStep::Item => "Choose item".into(),
            OpStep::Field => "Choose field".into(),
        };
        self.picker.scope = Some(self.crumb());
        self.picker.placeholder = match self.step {
            OpStep::Account => "Search accounts…".into(),
            OpStep::Vault => "Search vaults…".into(),
            OpStep::Item => "Search items…".into(),
            OpStep::Field => "Search fields…".into(),
        };
        self.picker.set_searchable(true);
        self.picker.status = PickerStatus::Loading(match self.step {
            OpStep::Account => "loading accounts…".into(),
            OpStep::Vault => "loading vaults…".into(),
            OpStep::Item => format!(
                "loading items from {}…",
                self.vault.as_ref().map(|v| v.1.as_str()).unwrap_or("vault")
            ),
            OpStep::Field => format!(
                "loading {}…",
                self.item.as_ref().map(|i| i.1.as_str()).unwrap_or("item")
            ),
        });
        self.picker.set_items(vec![]);
        self.loading_until = Some(now_ms + op.latency_ms());
    }

    pub fn tick(&mut self, now_ms: i64, op: &dyn OnePassword) -> Outcome {
        if let Some(t) = self.loading_until
            && now_ms >= t
        {
            self.loading_until = None;
            self.refresh(op);
            return Outcome::Changed;
        }
        Outcome::Ignored
    }

    fn set_error(&mut self, e: OpError) {
        let (msg, detail) = match &e {
            OpError::Locked => (
                "1Password is locked".to_owned(),
                Some("Unlock it in the 1Password app, then press r to retry".to_owned()),
            ),
            OpError::AuthorizationRequired { account } => (
                "Jackin is not authorized for 1Password CLI".to_owned(),
                Some(format!(
                    "Approve the request for {account} in 1Password, then press r to retry"
                )),
            ),
            OpError::PermissionDenied { vault } => (
                format!("No access to vault {vault}"),
                Some("Choose another vault".into()),
            ),
            other => (other.message(), None),
        };
        self.picker.status = PickerStatus::Error {
            message: msg,
            detail,
        };
        self.last_error = Some(e);
    }

    pub fn refresh(&mut self, op: &dyn OnePassword) {
        self.picker.status = PickerStatus::Ready;
        self.last_error = None;
        let q = self.picker.query().to_lowercase();
        let matches = |s: &str| q.is_empty() || s.to_lowercase().contains(&q);
        let items: Vec<Item> = match self.step {
            OpStep::Account => match op.list_accounts() {
                Ok(accts) => accts
                    .iter()
                    .filter(|a| matches(&a.id) || matches(&a.email))
                    .map(|a| {
                        Item::new(a.id.clone())
                            .detail(a.email.clone())
                            .glyph("▪")
                            .tag(match a.state {
                                crate::backend::OpAccountState::Available => "signed in",
                                crate::backend::OpAccountState::Locked => "locked",
                                crate::backend::OpAccountState::AuthorizationRequired => {
                                    "authorize"
                                }
                            })
                    })
                    .collect(),
                Err(e) => {
                    self.set_error(e);
                    return;
                }
            },
            OpStep::Vault => match op.list_vaults(self.account.as_deref().unwrap_or("")) {
                Ok(vaults) => vaults
                    .iter()
                    .filter(|v| matches(&v.name))
                    .map(|v| {
                        Item::new(v.name.clone())
                            .detail(format!("{} items", v.items.len()))
                            .glyph("▪")
                            .tag(match v.access {
                                crate::backend::VaultAccess::ReadWrite => "",
                                crate::backend::VaultAccess::ReadOnly => "read-only",
                                crate::backend::VaultAccess::Denied => "no access",
                            })
                    })
                    .collect(),
                Err(e) => {
                    self.set_error(e);
                    return;
                }
            },
            OpStep::Item => {
                let (vid, _) = self.vault.clone().unwrap_or_default();
                match op.list_items(self.account.as_deref().unwrap_or(""), &vid) {
                    Ok(items) => items
                        .iter()
                        .filter(|i| matches(&i.title))
                        .map(|i| {
                            Item::new(i.title.clone())
                                .detail(format!("{} · {} fields", i.category, i.fields.len()))
                                .glyph(" ")
                        })
                        .collect(),
                    Err(e) => {
                        self.set_error(e);
                        return;
                    }
                }
            }
            OpStep::Field => {
                let (vid, _) = self.vault.clone().unwrap_or_default();
                let (iid, _) = self.item.clone().unwrap_or_default();
                match op.list_fields(self.account.as_deref().unwrap_or(""), &vid, &iid) {
                    Ok(fields) => fields
                        .iter()
                        .filter(|f| matches(&f.label))
                        .map(|f| {
                            Item::new(f.label.clone())
                                .detail(f.kind.label().to_owned())
                                .glyph(if f.kind == FieldKind::Concealed {
                                    "•"
                                } else {
                                    " "
                                })
                                .disabled(self.concealed_only && f.kind != FieldKind::Concealed)
                        })
                        .collect(),
                    Err(e) => {
                        self.set_error(e);
                        return;
                    }
                }
            }
        };
        if items.is_empty() {
            self.picker.empty_text = "No matches".into();
        }
        self.picker.set_items(items);
    }

    fn back(&mut self, now_ms: i64, op: &dyn OnePassword) -> Outcome {
        match self.step {
            OpStep::Account => {
                self.result = Some(None);
            }
            OpStep::Vault => {
                if op.accounts().len() > 1 {
                    self.step = OpStep::Account;
                    self.account = None;
                    self.begin_load(now_ms, op);
                } else {
                    self.result = Some(None);
                }
            }
            OpStep::Item => {
                self.step = OpStep::Vault;
                self.vault = None;
                self.begin_load(now_ms, op);
            }
            OpStep::Field => {
                self.step = OpStep::Item;
                self.item = None;
                self.begin_load(now_ms, op);
            }
        }
        Outcome::Changed
    }

    fn choose(&mut self, i: usize, now_ms: i64, op: &dyn OnePassword) -> Outcome {
        let Some(row) = self.picker.items.get(i).cloned() else {
            return Outcome::Consumed;
        };
        match self.step {
            OpStep::Account => {
                self.account = Some(row.label);
                self.step = OpStep::Vault;
            }
            OpStep::Vault => {
                let vid = op
                    .list_vaults(self.account.as_deref().unwrap_or(""))
                    .ok()
                    .and_then(|vs| {
                        vs.iter()
                            .find(|v| v.name == row.label)
                            .map(|v| v.id.clone())
                    })
                    .unwrap_or_default();
                self.vault = Some((vid, row.label));
                self.step = OpStep::Item;
            }
            OpStep::Item => {
                let (vid, _) = self.vault.clone().unwrap_or_default();
                let iid = op
                    .list_items(self.account.as_deref().unwrap_or(""), &vid)
                    .ok()
                    .and_then(|is| {
                        is.iter()
                            .find(|i| i.title == row.label)
                            .map(|i| i.id.clone())
                    })
                    .unwrap_or_default();
                self.item = Some((iid, row.label));
                self.step = OpStep::Field;
            }
            OpStep::Field => {
                let (vid, _) = self.vault.clone().unwrap_or_default();
                let (iid, _) = self.item.clone().unwrap_or_default();
                match op.reference(
                    self.account.as_deref().unwrap_or(""),
                    &vid,
                    &iid,
                    &row.label,
                ) {
                    Ok(r) => match op.describe(&r) {
                        Ok(_) => self.result = Some(Some(r)),
                        Err(e) => self.set_error(e),
                    },
                    Err(e) => self.set_error(e),
                }
                return Outcome::Changed;
            }
        }
        self.begin_load(now_ms, op);
        Outcome::Changed
    }

    pub fn on_key(&mut self, key: &Key, now_ms: i64, op: &dyn OnePassword) -> Outcome {
        if key.is_char('r')
            && self.picker.status != PickerStatus::Ready
            && self.picker.query().is_empty()
        {
            self.begin_load(now_ms, op);
            return Outcome::Changed;
        }
        if self.loading_until.is_some() && !key.is(KeyCode::Esc) {
            return Outcome::Consumed;
        }
        let (o, ev) = self.picker.on_key(key);
        match ev {
            Some(PickerAction::QueryChanged) => {
                if (self.picker.status == PickerStatus::Ready
                    || matches!(self.picker.status, PickerStatus::Error { .. }))
                    && self.last_error.is_none()
                {
                    self.refresh(op);
                }
                Outcome::Changed
            }
            Some(PickerAction::Chosen(i)) | Some(PickerAction::ChosenAlt(i)) => {
                self.choose(i, now_ms, op)
            }
            Some(PickerAction::Back) | Some(PickerAction::Cancelled) => self.back(now_ms, op),
            _ => o.or(Outcome::Consumed),
        }
    }

    pub fn pointer(&mut self, ev: &PointerEvent, now_ms: i64, op: &dyn OnePassword) -> Outcome {
        let Some(r) = self.picker.route(ev) else {
            return match ev.wheel() {
                Some((Axis::Vertical, delta)) => {
                    self.picker.list.wheel(Axis::Vertical, delta).outcome()
                }
                _ if ev.is(Phase::Click) => Outcome::Consumed,
                _ => Outcome::Ignored,
            };
        };
        let (o, act) = r.into_parts();
        match act {
            Some(PickerAction::Chosen(i)) => self.choose(i, now_ms, op),
            Some(PickerAction::Cancelled) if self.dismiss.outside_click => {
                self.dismissed = true;
                Outcome::Changed
            }
            Some(_) => Outcome::Consumed,
            None if ev.is(Phase::Click) => o.or(Outcome::Consumed),
            None => o,
        }
    }

    pub fn hints(&self) -> Vec<Hint> {
        match (self.step, &self.picker.status) {
            (_, PickerStatus::Error { .. }) => vec![hint("r", "Retry"), hint("Esc", "Back")],
            (OpStep::Field, _) => vec![
                hint("↑↓", "Move"),
                hint("Enter", "Save reference"),
                hint("Esc", "Back to items"),
            ],
            (OpStep::Account, _) => vec![
                hint("↑↓", "Move"),
                hint("Enter", "Choose"),
                hint("Esc", "Cancel"),
            ],
            _ => vec![
                hint("↑↓", "Move"),
                hint("Enter", "Choose"),
                hint("Backspace", "Back"),
                hint("Esc", "Back"),
            ],
        }
    }

    pub fn render(&mut self, screen: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        self.picker.draw(screen, buf, ctx);
    }
}

impl Layer<ModalResult, World> for OpFlow {
    fn animating(&self) -> bool {
        true
    }
    fn spec(&self) -> LayerSpec {
        LayerSpec::modal(self.dismiss).initial_focus(self.picker.id)
    }
    fn render(&mut self, screen: Rect, buf: &mut Buffer, ctx: &mut RenderCtx, _: &World) {
        OpFlow::render(self, screen, buf, ctx);
    }
    fn on_key(&mut self, key: &Key, cx: &mut LayerCx<'_, World>) -> Outcome {
        OpFlow::on_key(self, key, cx.app.now_ms(), cx.app.op.as_ref()).or(Outcome::Consumed)
    }
    fn pointer(&mut self, ev: &PointerEvent, cx: &mut LayerCx<'_, World>) -> Outcome {
        OpFlow::pointer(self, ev, cx.app.now_ms(), cx.app.op.as_ref())
    }
    fn on_tick(&mut self, w: &World) -> Outcome {
        self.tick(w.now_ms(), w.op.as_ref())
    }
    fn hints(&self, _: Option<WidgetId>) -> Vec<Hint> {
        OpFlow::hints(self)
    }
    fn on_dismiss(&mut self, _: DismissReason) {
        self.dismissed = true;
    }
    fn take_result(&mut self) -> Option<LayerOut<ModalResult>> {
        if std::mem::take(&mut self.dismissed) {
            return Some(LayerOut::Close(ModalResult::Cancelled));
        }
        self.result
            .take()
            .map(|r| LayerOut::Close(ModalResult::Op(r)))
    }
}

pub use junie_tui::widgets::help::HelpOverlay;
