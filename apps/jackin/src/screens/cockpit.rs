use std::sync::Arc;

use crate::screens::Jackin;
use junie_tui::core::event::{Key, Outcome};
use junie_tui::core::id::WidgetId;
use junie_tui::core::intent::Phase;
use junie_tui::core::pointer::{MouseTarget, PointerEvent, scroll_input};
use junie_tui::shell::Page;
use junie_tui::theme::{Theme, Tone};
use junie_tui::ui::ctx::{RenderCtx, fill};
use junie_tui::ui::text::{plural, width};
use junie_tui::widgets::button::Button;
use junie_tui::widgets::dialog::Dialog;
use junie_tui::widgets::keyhint::{Hint, hint};
use junie_tui::widgets::notice::Notice;
use junie_tui::widgets::progress::spinner_frame;
use junie_tui::widgets::props::Prop;
use junie_tui::widgets::segments::Segment;
use junie_tui::widgets::steps::{Step, StepRail, StepState};
use junie_tui::widgets::viewport::{Span, TextViewport};
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};

use super::modals::{InfoDialog, InfoResult};
use super::{Cx, Go, LayerTag, ModalResult, Screen};
use crate::domain::account::AccountId;
use crate::domain::agent::Agent;
use crate::domain::launch::{LaunchContent, LaunchFailure, StageKind};
use crate::domain::precedence::PrecedenceLevel;
use crate::domain::workspace::WorkspaceId;
use crate::launch::{
    FailedLaunch, LaunchEvent, LaunchFeed, LaunchTarget, ReadyInstance, launch_accounts,
};
use crate::world::World;
use junie_tui::motion::Motion;

pub const RAIL: WidgetId = WidgetId::of("cockpit.rail");
pub const LOG: WidgetId = WidgetId::of("cockpit.log");
pub const CHIP_CONTAINER: WidgetId = WidgetId::of("cockpit.chip.container");
pub const CHIP_ACTIVITY: WidgetId = WidgetId::of("cockpit.chip.activity");
pub const CHIP_RUN: WidgetId = WidgetId::of("cockpit.chip.run");

pub const CANCEL_BTN: WidgetId = WidgetId::of("cockpit.cancel_btn");

/// What the feed's events have said so far.
struct Progress {
    states: Vec<StepState>,
    failure: Option<LaunchFailure>,
    done: bool,
    cancelled: bool,
}

impl Progress {
    fn new(stages: usize) -> Self {
        Self {
            states: vec![StepState::Queued; stages],
            failure: None,
            done: false,
            cancelled: false,
        }
    }

    fn counts(&self) -> (usize, usize) {
        let count = |wanted| self.states.iter().filter(|s| **s == wanted).count();
        (count(StepState::Done), count(StepState::Skipped))
    }

    fn blocked(&self) -> bool {
        self.states.contains(&StepState::Blocked)
    }

    fn is_terminal(&self) -> bool {
        self.done || self.failure.is_some() || self.blocked() || self.cancelled
    }
}

pub struct CockpitScreen {
    feed: Box<dyn LaunchFeed>,
    content: Arc<LaunchContent>,
    progress: Progress,
    pub workspace: Option<WorkspaceId>,
    pub role: String,
    pub agent: Agent,
    pub account: Option<AccountId>,

    pub accounts: Vec<AccountId>,
    pub rail: StepRail,
    pub activity: String,
    pub log: TextViewport,
    pub log_open: bool,
    pub container: Option<String>,
    pub credentials: Option<(String, String, Tone)>,
    pub tick: u64,
    pub motion: Motion,
    pub frozen_at: Option<u64>,
    pub instance_id: Option<String>,
    pub handoff_requested: bool,
    pub failure_shown: bool,
    log_area: Rect,
    pub debug: bool,
    pub credential_hold: bool,
    pub target_label: String,
    docker: bool,
}

impl CockpitScreen {
    pub fn new(w: &World, target: LaunchTarget, motion: Motion) -> Self {
        let feed = w.backend.launch(w, &target);
        let content = w.backend.launch_content();
        let accounts = launch_accounts(w, &target, &content);
        let LaunchTarget {
            workspace,
            role,
            agent,
            account,
        } = target;
        let steps = content.stages.iter().map(|s| Step::new(&s.label)).collect();
        let mut log = TextViewport::new(LOG).wrap(true).max_lines(5_000);
        log.follow = true;
        let target_label = match workspace.and_then(|id| w.workspace(id)) {
            Some(x) => format!("into workspace {}", x.name),
            None => format!("in directory {}", w.tilde(&w.cwd)),
        };
        let docker = w.docker_on(workspace, &role);
        Self {
            progress: Progress::new(content.stages.len()),
            feed,
            content,
            workspace,
            role,
            agent,
            account,
            accounts,
            rail: StepRail::new(RAIL, steps).selectable(false),
            activity: "Preparing launch…".into(),
            log,
            log_open: false,
            container: None,
            credentials: None,
            tick: 0,
            motion,
            frozen_at: None,
            instance_id: None,
            handoff_requested: false,
            failure_shown: false,
            log_area: Rect::ZERO,
            debug: false,
            credential_hold: false,
            target_label,
            docker,
        }
    }

    pub fn seek(&mut self, ticks: u64, w: &mut World, cx: &mut Cx) {
        for _ in 0..ticks {
            self.step(w, cx);
        }
    }

    fn build_stage(&self) -> Option<usize> {
        self.content.stages.first_of(|k| k == StageKind::BuildLog)
    }

    fn credentials_label(&self) -> &str {
        let stages = &self.content.stages;
        stages
            .first_of(|k| matches!(k, StageKind::Credentials { .. }))
            .map_or("Credentials", |i| stages.get(i).label.as_str())
    }

    fn credential_origin(&self, w: &World) -> (String, String, Tone) {
        let (origin, val, tone) = self.credential_origin_for(self.account.as_deref(), w);
        let others: Vec<String> = self
            .accounts
            .iter()
            .filter(|id| Some(id.as_str()) != self.account.as_deref())
            .filter_map(|id| w.accounts.get(id))
            .map(|a| a.title())
            .collect();
        let primary = self
            .account
            .as_ref()
            .and_then(|id| w.accounts.get(id))
            .map(|a| format!("{} ({})", a.title(), a.source.origin_label()))
            .unwrap_or_else(|| origin.clone());
        if others.is_empty() {
            let shown = if self.content.agent_provider_only {
                primary
            } else {
                origin
            };
            (shown, val, tone)
        } else {
            (
                format!(
                    "{} · {primary} · {}",
                    junie_tui::ui::text::plural(self.accounts.len().max(1), "account", "accounts"),
                    others.join(" · ")
                ),
                val,
                tone,
            )
        }
    }

    fn why_label(&self, account: &crate::domain::account::Account, w: &World) -> &'static str {
        let r = w.account_for(
            account.provider,
            self.workspace.and_then(|id| w.workspace(id)),
            Some(&self.role),
            None,
        );
        if r.account.as_deref() == Some(account.id.as_str()) {
            r.level.label()
        } else {
            PrecedenceLevel::Session.label()
        }
    }

    fn credential_origin_for(&self, account: Option<&str>, w: &World) -> (String, String, Tone) {
        match account.and_then(|id| w.accounts.get(id)) {
            Some(a) => {
                let why = self.why_label(a, w);
                let origin = format!(
                    "{} ({} · {})",
                    a.title(),
                    a.source.origin_label(),
                    a.source.safe_detail()
                );
                let (val, tone) = match a.validation.level() {
                    Some(l) => (format!("{} · {why}", l.label()), Tone::Secondary),
                    None => (format!("{} · {why}", a.validation.label()), Tone::Warning),
                };
                (origin, val, tone)
            }
            None => (
                format!("host profile · {} sync", self.agent.label()),
                format!("{} · no registered account", PrecedenceLevel::None.label()),
                Tone::Warning,
            ),
        }
    }

    fn step(&mut self, w: &mut World, cx: &mut Cx) -> bool {
        if self.progress.is_terminal() || self.handoff_requested {
            return false;
        }
        self.tick += 1;
        let events = self.feed.poll(w);
        let mut changed = !events.is_empty();
        for ev in events {
            match ev {
                LaunchEvent::StageChanged(stage, state) => {
                    if let Some(slot) = self.progress.states.get_mut(stage) {
                        *slot = state;
                    }
                    self.rail.set_state(stage, state);
                    if state == StepState::Done {
                        self.rail.set_meta(
                            stage,
                            Some(format!(
                                "{:.1} s",
                                self.content.stages.get(stage).ticks as f64 * 0.033 * 3.0
                            )),
                        );
                    }
                    if state == StepState::Running
                        && matches!(
                            self.content.stages.get(stage).kind,
                            StageKind::Credentials { .. }
                        )
                    {
                        let (o, v, tone) = self.credential_origin(w);
                        self.credentials = Some((o, v, tone));
                    }
                }
                LaunchEvent::Activity(a) => self.activity = a,
                LaunchEvent::BuildLine(l) => self.log.push(ansi_line(&l)),
                LaunchEvent::ContainerReady(c) => self.container = Some(c),
                LaunchEvent::CredentialsResolved { .. } => {
                    if let Some((_, v, tone)) = self.credentials.as_mut() {
                        *v = format!("{} · {v}", self.content.words.credentials_resolved);
                        *tone = Tone::Secondary;
                    }
                }
                LaunchEvent::CredentialError { message } => {
                    self.credential_hold = true;
                    if let Some((_, v, tone)) = self.credentials.as_mut() {
                        *v = message.clone();
                        *tone = Tone::Error;
                    }
                    let mut d = Dialog::destructive(
                        WidgetId::of("cockpit.cred"),
                        "Credential source unavailable",
                        &format!(
                            "{message}. The launch is paused at the {} stage; nothing was injected.",
                            self.credentials_label()
                        ),
                        "Cancel launch",
                    );
                    d.actions = vec![
                        Button::secondary(WidgetId::of("cockpit.cred").sub("retry"), "Retry"),
                        Button::secondary(
                            WidgetId::of("cockpit.cred").sub("plain"),
                            "Enter plain text instead",
                        ),
                        Button::danger(WidgetId::of("cockpit.cred").sub("cancel"), "Cancel launch"),
                    ];
                    d.cancel_index = Some(2);
                    d.initial_focus = d.actions[0].id;
                    d.width = 70;
                    cx.open(d, LayerTag::new("cred"));
                }
                LaunchEvent::Failed(failed) => {
                    let FailedLaunch { failure, instance } = *failed;
                    self.frozen_at = Some(self.tick);
                    self.activity =
                        format!("{} failed", self.content.stages.get(failure.stage).label);
                    self.log_open = false;
                    self.progress.failure = Some(failure);
                    self.instance_id = Some(instance.id.clone());
                    w.admit(instance);
                    self.open_failure(w, cx);
                }
                LaunchEvent::Ready(ready) => {
                    let ReadyInstance { instance, .. } = *ready;
                    self.progress.done = true;
                    self.activity = self.content.words.ready_activity.clone();
                    let id = instance.id.clone();
                    w.admit(instance);
                    self.handoff_requested = true;
                    cx.go(Go::Attach {
                        instance: id,
                        pane: None,
                    });
                }
            }
            changed = true;
        }
        changed
    }

    fn open_failure(&mut self, w: &World, cx: &mut Cx) {
        let Some(f) = self.progress.failure.clone() else {
            return;
        };
        self.failure_shown = true;
        let words = &self.content.words;
        let props = vec![
            Prop::new("Stage", self.content.stages.get(f.stage).label.clone()).tone(Tone::Error),
            Prop::new("Run id", self.feed.run_id().to_owned()).copyable(),
            Prop::new("Next step", f.next_step.clone()).wrap(),
            Prop::new(
                &words.unit,
                self.container
                    .clone()
                    .unwrap_or_else(|| words.unit_missing.clone()),
            ),
        ];
        let title = match self.content.stages.get(f.stage).kind {
            StageKind::BuildLog => words.build_failed.clone(),
            StageKind::Credentials { .. } => "Credential check failed".to_owned(),
            StageKind::Plain => "Launch failed".to_owned(),
        };
        let d = InfoDialog::new(WidgetId::of("cockpit.failure"), &title, props)
            .error()
            .intro(vec![
                (f.summary.clone(), Tone::Normal),
                (
                    format!("Loading {} {}", self.role, self.target_label),
                    Tone::Muted,
                ),
            ])
            .detail(f.detail.clone())
            .meta("run id is the only copyable value")
            .width(70);
        let _ = w;
        cx.open(d, LayerTag::new("failure"));
    }

    fn open_info(&mut self, w: &World, cx: &mut Cx) {
        let mut props = vec![
            Prop::new("Target", format!("{} {}", self.role, self.target_label)),
            Prop::new("Role", self.role.clone()),
            Prop::new(
                "Agent",
                format!(
                    "{} · account {}",
                    self.agent.label(),
                    self.account
                        .as_ref()
                        .and_then(|id| w.accounts.get(id))
                        .map(|a| a.title())
                        .unwrap_or("host profile".into())
                ),
            ),
            Prop::new("Run id", self.feed.run_id().to_owned()).copyable(),
            Prop::new(
                "jackin",
                w.surfaces
                    .cockpit_version
                    .as_deref()
                    .unwrap_or(&w.surfaces.version),
            ),
        ];
        if let Some(c) = &self.container {
            props.insert(0, Prop::new(&self.content.words.unit, c.clone()).copyable());
        }
        if self.debug {
            props.push(Prop::new(
                "Telemetry",
                format!(
                    "run {} -> otlp://collector.internal:4317",
                    self.feed.run_id()
                ),
            ));
        }
        let mut d =
            InfoDialog::new(WidgetId::of("cockpit.info"), "Debug info", props).meta("read-only");
        if self.docker
            && let Some(risks) = w.surfaces.docker.as_ref().map(|d| &d.risks)
        {
            d = d
                .detail(
                    risks
                        .iter()
                        .map(|r| format!("{} · {}", r.label, r.text))
                        .collect(),
                )
                .width(80);
        }
        cx.open(d, LayerTag::new("info"));
    }

    fn open_quit(&mut self, cx: &mut Cx) {
        let d = Dialog::destructive(
            WidgetId::of("cockpit.quit"),
            "Quit jackin❯?",
            "Quitting force-stops the launch immediately. Partially prepared resources are cleaned up on the next launch.",
            "Quit",
        );
        cx.open(d, LayerTag::new("quit"));
    }

    /// Ends the launch and keeps the record the backend made of it.
    fn abandon(&mut self, w: &mut World) {
        self.progress.cancelled = true;
        if let Some(record) = self.feed.cancel(w) {
            self.instance_id = Some(record.id.clone());
            w.admit(record);
        }
    }

    fn cancel(&mut self, w: &mut World, cx: &mut Cx) {
        if self.progress.is_terminal() {
            return;
        }
        self.abandon(w);
        self.frozen_at = Some(self.tick);
        self.activity = "Launch cancelled".into();
        cx.status("Launch cancelled · nothing was attached");
        cx.go(Go::LaunchFailedAck {
            instance: self.instance_id.clone(),
        });
    }

    fn ask_cancel(&mut self, cx: &mut Cx) -> Outcome {
        if self.progress.is_terminal() {
            return Outcome::Consumed;
        }
        let d = Dialog::destructive(
            WidgetId::of("cockpit.cancel"),
            "Cancel the launch?",
            "The pipeline stops at its current stage and the partially prepared instance is marked failed setup. Nothing is attached.",
            "Cancel launch",
        )
        .cancel_label("Keep launching");
        cx.open(d, LayerTag::new("cancel"));
        Outcome::Changed
    }

    fn draw_log(&mut self, screen: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let active = !self.progress.is_terminal()
            && self
                .rail
                .frontier()
                .zip(self.build_stage())
                .is_some_and(|(i, build)| i <= build);
        let title = if active {
            format!("{} · building…", self.content.words.log_title)
        } else {
            self.content.words.log_title.clone()
        };
        let n = self.log.len();
        let meta = if self.log.follow {
            format!("{n} lines · following")
        } else {
            format!("{n} lines")
        };
        let w = screen.width.saturating_sub(6).min(110);
        let h = screen.height.saturating_sub(4);

        let (_, inner) = junie_tui::ui::modal::Frame::new(w, h)
            .title(&title)
            .meta(Some(&meta))
            .placement(junie_tui::ui::popup::Placement::Center)
            .dismiss(junie_tui::ui::popup::Dismiss::LIGHT)
            .render(screen, buf, ctx);
        let t = ctx.theme;
        let bg = t.surface_elevated;
        let body = Rect::new(
            inner.x,
            inner.y,
            inner.width,
            inner.height.saturating_sub(2),
        );
        self.log_area = body;
        if self.log.is_empty() {
            buf.set_string(
                body.x,
                body.y,
                &self.content.words.log_waiting,
                t.muted().bg(bg),
            );
        } else {
            self.log.render(body, buf, ctx, bg);
        }
        let hint = "↑↓ Scroll · PgUp PgDn Page · End Follow · Esc Close";
        buf.set_string(
            inner.x,
            inner.bottom() - 1,
            junie_tui::ui::reveal::cut(
                Rect::new(inner.x, inner.bottom() - 1, inner.width, 1),
                hint,
            ),
            t.faint().bg(bg),
        );
        ctx.hits.register(LOG, body);
        ctx.hits.register(
            junie_tui::widgets::scrollbar::id_for(LOG),
            Rect::new(body.right() - 1, body.y, 1, body.height),
        );
    }
}

pub fn ansi_line(l: &str) -> Vec<Span> {
    let mut spans = vec![];
    if let Some(rest) = l.strip_prefix('#') {
        let end = rest.find(' ').unwrap_or(rest.len());
        spans.push(Span::muted(format!("#{}", &rest[..end])));
        let body = &rest[end..];
        let tone = if body.contains("DONE") || body.contains("CACHED") {
            Tone::Secondary
        } else if body.contains("warning") {
            Tone::Warning
        } else if body.contains("error") || body.contains("Error") {
            Tone::Error
        } else {
            Tone::Normal
        };
        spans.push(Span::new(body.to_owned(), tone));
    } else {
        spans.push(Span::plain(l.to_owned()));
    }
    spans
}

fn wrapped_arrow(buf: &mut Buffer, area: Rect, t: &Theme) {
    let _ = (buf, area, t);
}

impl Page<Jackin> for CockpitScreen {
    fn enter(&mut self, _w: &mut World, cx: &mut Cx) {
        cx.focus.focus(RAIL);
    }

    fn primary_focus(&self) -> Option<WidgetId> {
        Some(RAIL)
    }

    fn animating(&self, _w: &World) -> bool {
        !self.progress.is_terminal() || self.frozen_at.is_none()
    }

    fn on_tick(&mut self, w: &mut World, cx: &mut Cx) -> Outcome {
        if !w.clock.running {
            return Outcome::Ignored;
        }
        if self.motion == Motion::Reduced {
            let mut changed = false;
            for _ in 0..3 {
                changed |= self.step(w, cx);
            }
            return if changed {
                Outcome::Changed
            } else {
                Outcome::Consumed
            };
        }
        let changed = self.step(w, cx);
        if changed || self.frozen_at.is_none() {
            Outcome::Changed
        } else {
            Outcome::Ignored
        }
    }

    fn on_key(&mut self, key: &Key, w: &mut World, cx: &mut Cx) -> Outcome {
        if key.ctrl_char('c') && !(self.log_open && self.log.has_selection()) {
            cx.status("Aborted · terminal restored, cleanup deferred");
            self.abandon(w);
            cx.go(Go::LaunchFailedAck {
                instance: self.instance_id.clone(),
            });
            return Outcome::Changed;
        }
        if key.ctrl_char('q') {
            self.open_quit(cx);
            return Outcome::Changed;
        }
        if self.log_open {
            match key.code {
                KeyCode::Esc | KeyCode::Char('b') | KeyCode::Char('q') => {
                    self.log_open = false;
                    cx.focus.focus(RAIL);
                    return Outcome::Changed;
                }
                _ => {
                    let (o, ev) = self.log.on_key(key);
                    if let Some(junie_tui::widgets::viewport::ViewportAction::Copy(text)) = ev {
                        cx.copy(text);
                    }
                    return o.or(Outcome::Consumed);
                }
            }
        }
        match key.code {
            KeyCode::Char('b') | KeyCode::Enter if key.plain() => {
                if self.log.is_empty()
                    && self
                        .rail
                        .frontier()
                        .zip(self.build_stage())
                        .is_some_and(|(i, build)| i < build)
                {
                    cx.status("No build output yet · the derived image stage has not started");
                    return Outcome::Changed;
                }
                self.log_open = true;
                self.log.set_follow(true);
                cx.focus.focus(LOG);
                Outcome::Changed
            }
            KeyCode::Char('i') if key.plain() => {
                self.open_info(w, cx);
                Outcome::Changed
            }
            KeyCode::Char('x') if key.plain() => self.ask_cancel(cx),
            KeyCode::Char('t') if key.plain() => {
                self.debug = !self.debug;
                Outcome::Changed
            }
            KeyCode::Esc if self.progress.failure.is_some() && !self.failure_shown => {
                self.open_failure(w, cx);
                Outcome::Changed
            }
            KeyCode::Esc | KeyCode::Enter if self.progress.blocked() => {
                cx.status("This stage is blocked · x cancels the launch, Ctrl+Q quits");
                Outcome::Changed
            }
            _ => Outcome::Consumed,
        }
    }

    fn pointer(&mut self, ev: &PointerEvent, w: &mut World, cx: &mut Cx) -> Outcome {
        if self.log_open {
            if let Some(r) = self.log.route(ev) {
                return r.outcome();
            }
            if ev.id == junie_tui::ui::modal::SURFACE
                && let Some(r) = scroll_input(&mut self.log, ev)
            {
                return r.outcome();
            }
        }
        if !ev.is(Phase::Click) {
            return Outcome::Ignored;
        }
        let id = ev.id;
        if self.log_open && id == junie_tui::ui::modal::CLOSE {
            self.log_open = false;
            cx.focus.focus(RAIL);
            return Outcome::Changed;
        }
        if id == CANCEL_BTN {
            return self.ask_cancel(cx);
        }
        if id == CHIP_ACTIVITY && !self.log.is_empty() {
            self.log_open = true;
            cx.focus.focus(LOG);
            return Outcome::Changed;
        }
        if id == CHIP_CONTAINER || id == CHIP_RUN {
            self.open_info(w, cx);
            return Outcome::Changed;
        }
        Outcome::Consumed
    }

    fn on_layer_result(
        &mut self,
        tag: &LayerTag,
        result: ModalResult,
        w: &mut World,
        cx: &mut Cx,
    ) -> Outcome {
        match (tag.kind, result) {
            ("failure", ModalResult::Info(InfoResult::Copy(v))) => {
                cx.copy(v);
                cx.status("Run id copied");

                self.open_failure(w, cx);
                Outcome::Changed
            }
            ("failure", _) => {
                cx.go(Go::LaunchFailedAck {
                    instance: self.instance_id.clone(),
                });
                Outcome::Changed
            }
            ("info", ModalResult::Info(InfoResult::Copy(v))) => {
                cx.copy(v);
                Outcome::Changed
            }
            (
                "quit",
                ModalResult::Dialog {
                    action: Some(1), ..
                },
            ) => {
                self.abandon(w);
                cx.go(Go::Quit);
                Outcome::Changed
            }
            (
                "cancel",
                ModalResult::Dialog {
                    action: Some(1), ..
                },
            ) => {
                self.cancel(w, cx);
                Outcome::Changed
            }
            ("cred", ModalResult::Dialog { action, .. }) => match action {
                Some(0) => {
                    self.credential_hold = false;
                    self.feed.retry_credentials(w.op.as_mut());
                    if let Some((_, v, tone)) = self.credentials.as_mut() {
                        *v = "retrying after unlock…".into();
                        *tone = Tone::Secondary;
                    }
                    cx.status("1Password unlocked · resolving the reference again");
                    Outcome::Changed
                }
                Some(1) => {
                    self.credential_hold = false;
                    self.account = None;
                    self.feed.use_plain_text();
                    if let Some((o, v, tone)) = self.credentials.as_mut() {
                        *o = format!(
                            "plain text · masked API key · {}",
                            crate::domain::account::masked("Q1zx")
                        );
                        *v = "typed once for this launch · never stored".into();
                        *tone = Tone::Secondary;
                    }
                    cx.status("Using a plain-text key for this launch only");
                    Outcome::Changed
                }
                _ => {
                    self.cancel(w, cx);
                    Outcome::Changed
                }
            },
            _ => Outcome::Changed,
        }
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx, w: &World) {
        let t = ctx.theme;
        fill(buf, area, t.base());

        let rail_h = (self.content.stages.len() as u16).min(area.height.saturating_sub(9));
        let rail_w = 44u16.min(area.width.saturating_sub(4));
        let rail_x = area.x + (area.width.saturating_sub(rail_w)) / 2;
        let ident_y = area.y + 1;
        let rail_y = ident_y + 5;
        let rail = Rect::new(rail_x, rail_y, rail_w, rail_h);

        let exclude = [
            Rect::new(
                rail_x.saturating_sub(2),
                ident_y,
                rail_w + 4,
                rail_h + 5 + 1,
            ),
            Rect::new(area.x, area.bottom().saturating_sub(3), area.width, 3),
        ];
        let running = !self.progress.is_terminal() && self.frozen_at.is_none();
        let t_local = match self.motion {
            Motion::Reduced => 60,
            _ => self.frozen_at.unwrap_or(self.tick),
        };
        crate::rain::paint_atmosphere(
            buf,
            area,
            &exclude,
            t_local,
            running,
            self.frozen_at.is_some(),
            t,
        );

        let ws_name = self.target_label.clone();
        let head = if self.rail.frontier().is_none() && self.tick == 0 {
            "Preparing launch…".to_owned()
        } else {
            format!("Loading {} {ws_name}", self.role)
        };
        let center = |buf: &mut Buffer, y: u16, text: &str, style: Style| {
            let n = width(text) as u16;
            let x = area.x + area.width.saturating_sub(n) / 2;
            buf.set_string(
                x,
                y,
                junie_tui::ui::reveal::cut(Rect::new(x, y, area.width, 1), text),
                style,
            );
        };
        center(
            buf,
            ident_y,
            " jackin❯ ",
            Style::new()
                .fg(t.text_on_accent)
                .bg(t.accent)
                .add_modifier(Modifier::BOLD),
        );
        center(buf, ident_y + 1, &head, t.title());
        let acc = self
            .account
            .as_ref()
            .and_then(|id| w.accounts.get(id))
            .map(|a| a.title())
            .unwrap_or("host profile".into());
        let why = self
            .account
            .as_ref()
            .and_then(|id| w.accounts.get(id))
            .map(|a| self.why_label(a, w))
            .unwrap_or("no registered account");
        center(
            buf,
            ident_y + 2,
            &format!(
                "{} · {} · account {acc} ({why})",
                self.agent.label(),
                self.agent.provider().label()
            ),
            t.secondary(),
        );
        let (done, skipped) = self.progress.counts();
        let frontier = self
            .rail
            .frontier()
            .map(|i| {
                format!(
                    "stage {} of {} · {}",
                    i + 1,
                    self.content.stages.len(),
                    self.content.stages.get(i).label
                )
            })
            .unwrap_or_else(|| {
                if self.progress.done {
                    "all stages complete".into()
                } else {
                    "stopped".into()
                }
            });
        center(
            buf,
            ident_y + 3,
            &format!("{frontier} · {done} done · {skipped} skipped"),
            t.muted(),
        );

        fill(
            buf,
            Rect::new(rail.x - 1, rail.y, rail.width + 2, rail.height),
            t.base(),
        );
        self.rail.render(rail, buf, ctx, t.canvas);

        let mut y = rail.bottom() + 1;
        if let Some((origin, val, tone)) = &self.credentials
            && y + 1 < area.bottom().saturating_sub(3)
        {
            let line = format!("credentials  {origin}");
            let x = rail.x.saturating_sub(4);
            let wdt = area.right().saturating_sub(x + 2) as usize;
            fill(buf, Rect::new(area.x, y, area.width, 2), t.base());
            buf.set_string(
                x,
                y,
                junie_tui::ui::reveal::cut(Rect::new(x, y, (wdt) as u16, 1), &line),
                t.muted(),
            );
            buf.set_string(
                x + 13,
                y + 1,
                junie_tui::ui::reveal::cut(
                    Rect::new(x + 13, y + 1, (wdt.saturating_sub(13)) as u16, 1),
                    val,
                ),
                Style::new().fg(t.tone(*tone)),
            );
            y += 3;
        }
        if self.docker
            && let Some(d) = w.surfaces.docker.as_ref()
        {
            let ny = rail.bottom() + 4;
            let x = rail.x.saturating_sub(4);
            let width = area.right().saturating_sub(x + 2);
            let notice = Notice::warning(format!(
                "Docker is on · the agent can read the engine key and containers bypass the sandbox network policy · i shows all {}",
                plural(d.risks.len(), "risk", "risks")
            ));
            let h = notice.height(width);
            if ny + h < area.bottom().saturating_sub(3) {
                fill(buf, Rect::new(area.x, ny, area.width, h), t.base());
                notice.render(Rect::new(x, ny, width, h), buf, ctx, t.canvas);
            }
        }
        let _ = y;

        let ay = area.bottom().saturating_sub(2);
        fill(buf, Rect::new(area.x, ay - 1, area.width, 3), t.base());
        let spinner = if running {
            format!("{} ", spinner_frame(self.tick))
        } else if self.progress.failure.is_some() {
            "! ".into()
        } else {
            "".into()
        };
        let activity = format!("{spinner}{}", self.activity);
        let act_style = if self.progress.failure.is_some() {
            t.error_fg()
        } else {
            t.secondary()
        };

        let act_style = if self.log.is_empty() {
            act_style
        } else {
            let link = ctx.title_style(CHIP_ACTIVITY, t.canvas);
            let ink = act_style.fg.unwrap_or(t.text_secondary);
            link.fg(t.legible(ink, link.bg.unwrap_or(t.canvas)))
        };
        let ax = area.x + 1;
        buf.set_string(
            ax,
            ay,
            junie_tui::ui::reveal::cut(
                Rect::new(ax, ay, area.width.saturating_sub(2), 1),
                &activity,
            ),
            act_style,
        );
        if !self.log.is_empty() {
            let lines = format!("{} lines · b build log", self.log.len());
            buf.set_string(
                ax,
                ay + 1,
                junie_tui::ui::reveal::cut(
                    Rect::new(ax, ay + 1, area.width.saturating_sub(2), 1),
                    &lines,
                ),
                t.faint(),
            );
            ctx.clickable(CHIP_ACTIVITY, Rect::new(ax, ay, width(&activity) as u16, 2));
        }
        let mut rx = area.right().saturating_sub(1);
        if self.debug {
            let chip = format!(" {} ", self.feed.run_id());
            let cw = width(&chip) as u16;
            rx = rx.saturating_sub(cw);

            let vs = ctx.state(CHIP_RUN);
            let chrome = ctx.title_style(CHIP_RUN, t.canvas);
            let st = chrome
                .patch(t.link_style(vs.hovered || vs.focused))
                .fg(t.legible(t.link, chrome.bg.unwrap_or(t.canvas)));
            buf.set_string(rx, ay, &chip, st);
            ctx.clickable(CHIP_RUN, Rect::new(rx, ay, cw, 1));
            rx = rx.saturating_sub(2);
        }
        if let Some(c) = &self.container {
            let chip = format!(" {c} ");
            let cw = width(&chip) as u16;
            if rx > cw + width(&activity) as u16 + 4 {
                rx = rx.saturating_sub(cw);
                let st = ctx.title_style(CHIP_CONTAINER, t.canvas);
                buf.set_string(rx, ay, &chip, st);
                ctx.clickable(CHIP_CONTAINER, Rect::new(rx, ay, cw, 1));
            }
        }

        if !self.progress.is_terminal() {
            let mut b = Button::secondary(CANCEL_BTN, "Cancel launch…");
            let bw = b.width();
            let bx = area.right().saturating_sub(bw + 1);
            if bx > area.x + 24 {
                b.render(Rect::new(bx, ay + 1, bw, 1), buf, ctx, t.canvas);
            }
        }
        wrapped_arrow(buf, area, t);
        ctx.control(RAIL, Rect::new(rail.x, rail.y, 1, 1), false);
        if self.log_open {
            self.draw_log(area, buf, ctx);
        }
    }

    fn hints(&self, _focus: Option<WidgetId>, _w: &World) -> Vec<Hint> {
        if self.log_open {
            return vec![
                hint("↑↓", "Scroll"),
                hint("End", "Follow"),
                hint("Esc", "Close log"),
            ];
        }
        let mut v = vec![];
        if !self.log.is_empty() {
            v.push(hint("b", "Build log"));
        }
        v.push(Hint::new("i", format!("{} info", self.content.words.unit)));
        if !self.progress.is_terminal() {
            v.push(hint("x", "Cancel launch…"));
        }
        v.push(hint("Ctrl+Q", "Quit"));
        v.push(hint("Ctrl+C", "Abort now"));
        v
    }

    fn crumb(&self, _w: &World) -> String {
        format!(
            "Launch › {} › {}",
            self.target_label.rsplit(' ').next().unwrap_or(""),
            self.role
        )
    }

    fn on_esc_top(&mut self, _w: &mut World, cx: &mut Cx) -> Outcome {
        cx.status("Esc does nothing here · x cancels the launch, Ctrl+Q quits");
        Outcome::Changed
    }
}

impl Screen for CockpitScreen {
    fn strip_right(&self, _w: &World) -> Vec<Segment> {
        let (done, _) = self.progress.counts();
        vec![
            Segment::new(
                format!("{done}/{} stages", self.content.stages.len()),
                Tone::Muted,
            )
            .priority(5),
        ]
    }
}
