mod common;

use common::*;
use junie_tui::core::focus::{Focus, FocusRing};
use junie_tui::core::hit::HitRegistry;
use junie_tui::core::id::WidgetId;
use junie_tui::core::intent::DismissReason;
use junie_tui::theme::{ColorLevel, Theme};
use junie_tui::ui::ctx::{Interaction, RenderCtx};
use junie_tui::ui::layer::{Layer, LayerCx, LayerOut};
use junie_tui::widgets::diff::{DiffFile, DiffHunk, DiffLine, DiffStatus, UnreadFile};
use junie_tui::widgets::inspect_changes::{ChangeSet, InspectChanges, InspectMode, InspectOutcome};
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::Rect;

const ID: WidgetId = WidgetId::of("t.inspect");
const SCREEN: Rect = Rect::new(0, 0, 120, 36);

#[derive(Debug, PartialEq, Eq)]
struct Out(InspectOutcome);

impl From<InspectOutcome> for Out {
    fn from(o: InspectOutcome) -> Self {
        Self(o)
    }
}

fn file(path: &str) -> DiffFile {
    DiffFile {
        path: path.into(),
        status: DiffStatus::Modified,
        hunks: vec![DiffHunk {
            old_start: 1,
            new_start: 1,
            lines: vec![DiffLine::context("same"), DiffLine::add("new line")],
        }],
    }
}

fn unread(path: &str) -> UnreadFile {
    UnreadFile {
        path: path.into(),
        status: DiffStatus::Modified,
    }
}

fn changes(files: Vec<DiffFile>, unread: Vec<UnreadFile>) -> ChangeSet {
    ChangeSet {
        files,
        unpushed: 0,
        omitted: 0,
        unread,
    }
}

fn render(ic: &mut InspectChanges) -> Buffer {
    let theme = Theme::for_level(ColorLevel::TrueColor);
    let mut buf = Buffer::empty(SCREEN);
    let mut hits = HitRegistry::default();
    let mut ring = FocusRing::default();
    let mut ctx = RenderCtx::new(&theme, Interaction::default(), &mut hits, &mut ring);
    Layer::<Out, ()>::render(ic, SCREEN, &mut buf, &mut ctx, &());
    buf
}

fn screen_text(buf: &Buffer) -> String {
    (0..buf.area.height)
        .map(|y| row(buf, y))
        .collect::<Vec<_>>()
        .join("\n")
}

fn closed_with(ic: &mut InspectChanges) -> Option<InspectOutcome> {
    match Layer::<Out, ()>::take_result(ic)? {
        LayerOut::Close(Out(o)) | LayerOut::Event(Out(o)) => Some(o),
    }
}

#[test]
fn the_summary_counts_unread_files_that_the_list_shows() {
    let set = changes(vec![], vec![unread("a.bin"), unread("b.bin")]);

    assert_eq!(set.summary(), "2 files · +0 −0");
    assert_eq!(
        changes(vec![file("a.rs")], vec![unread("b.bin")]).summary(),
        "2 files · +1 −0"
    );
}

#[test]
fn the_advanced_header_names_the_key_that_switches_the_diff() {
    let mut ic = InspectChanges::new(
        ID,
        "Changes",
        changes(vec![file("a.rs")], vec![]),
        InspectMode::Advanced,
    );
    let text = screen_text(&render(&mut ic));
    assert!(text.contains("unified · v switches"), "{text}");
    assert!(!text.contains("d switches"));

    let mut focus = Focus::default();
    let ring = FocusRing::default();
    let mut cx = LayerCx {
        focus: &mut focus,
        ring: &ring,
        app: &(),
    };
    Layer::<Out, ()>::on_key(&mut ic, &key(KeyCode::Char('v')), &mut cx);
    let text = screen_text(&render(&mut ic));
    assert!(text.contains("review · v switches"), "{text}");
}

fn opened(returns_to_exit: bool) -> InspectChanges {
    let mut ic = InspectChanges::new(
        ID,
        "Changes",
        changes(vec![file("a.rs")], vec![]),
        InspectMode::Compact,
    )
    .returns_to_exit(returns_to_exit);
    render(&mut ic);
    ic
}

#[test]
fn a_dismissal_resolves_like_leaving_with_escape() {
    for (returns_to_exit, expected) in
        [(true, InspectOutcome::Back), (false, InspectOutcome::Close)]
    {
        let mut dismissed = opened(returns_to_exit);
        Layer::<Out, ()>::on_dismiss(&mut dismissed, DismissReason::OutsideClick);

        let mut escaped = opened(returns_to_exit);
        let mut focus = Focus::default();
        let ring = FocusRing::default();
        let mut cx = LayerCx {
            focus: &mut focus,
            ring: &ring,
            app: &(),
        };
        Layer::<Out, ()>::on_key(&mut escaped, &key(KeyCode::Esc), &mut cx);

        assert_eq!(closed_with(&mut dismissed), Some(expected));
        assert_eq!(closed_with(&mut escaped), Some(expected));
    }
}
