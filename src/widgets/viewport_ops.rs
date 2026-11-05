use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use ratatui::style::Color;

use crate::core::event::Outcome;
use crate::core::hit::{Axis, Bar};
use crate::core::id::WidgetId;
use crate::core::scroll::{ScrollPane, Scrollable};
use crate::ui::ctx::RenderCtx;
use crate::widgets::viewport::{CellPos, TextViewport};

/// What a host needs from a scrolling, selectable text view, without knowing
/// whether the text is a list of styled lines or a terminal grid.
///
/// The Capsule drives every pane through this trait. `TextViewport`
/// implements it by delegation, so the trait adds no behaviour of its own.
pub trait ViewportOps {
    fn id(&self) -> WidgetId;
    fn set_area(&mut self, area: Rect);
    fn hbar(&self) -> Rect;
    fn bar_input(&mut self, bar: &Bar, pos: Position, pressed: bool) -> Outcome;
    fn scroll(&self) -> &ScrollPane;

    fn follow(&self) -> bool;
    fn set_follow(&mut self, on: bool);
    fn wheel(&mut self, axis: Axis, delta: i32) -> Outcome;
    fn page_up(&mut self);
    /// Pages down, and follows the live tail again when the page reached it.
    fn page_down(&mut self);
    fn jump_start(&mut self);

    fn has_selection(&self) -> bool;
    fn selected_text(&self) -> Option<String>;
    fn clear_selection(&mut self) -> Outcome;
    fn select_word_at(&mut self, pos: Position) -> Outcome;
    fn on_click(&mut self, pos: Position) -> Outcome;
    fn on_drag(&mut self, pos: Position) -> Outcome;
    fn context_point(&self) -> Position;
    /// Whether the `y` key copies the selection. A viewport that copies on
    /// drag release only answers `false`, so a host hides the shortcut.
    fn keyboard_copy(&self) -> bool;
    /// Whether `draw` paints the vertical scroll track itself. A host draws
    /// the bar of a view that answers `false`.
    fn draws_track(&self) -> bool {
        false
    }

    fn caret(&self) -> Option<CellPos>;
    fn caret_visible(&self) -> bool;
    fn screen_pos(&self, at: CellPos) -> Option<Position>;

    /// Paints the view into `area` and returns where the hardware cursor
    /// belongs, if it is shown. `caret_allowed` is the host's say: a dialog,
    /// a drag or another focus turns it off. The view keeps its state, so a
    /// host can draw from a shared reference.
    fn draw(
        &self,
        area: Rect,
        buf: &mut Buffer,
        ctx: &mut RenderCtx,
        bg: Color,
        caret_allowed: bool,
    ) -> Option<Position>;
}

impl ViewportOps for TextViewport {
    fn id(&self) -> WidgetId {
        self.id
    }

    fn set_area(&mut self, area: Rect) {
        TextViewport::set_area(self, area);
    }

    fn hbar(&self) -> Rect {
        TextViewport::hbar(self)
    }

    fn bar_input(&mut self, bar: &Bar, pos: Position, pressed: bool) -> Outcome {
        Scrollable::bar_input(self, bar, pos, pressed).outcome()
    }

    fn scroll(&self) -> &ScrollPane {
        &self.scroll
    }

    fn follow(&self) -> bool {
        self.follow
    }

    fn set_follow(&mut self, on: bool) {
        TextViewport::set_follow(self, on);
    }

    fn wheel(&mut self, axis: Axis, delta: i32) -> Outcome {
        Scrollable::wheel(self, axis, delta).outcome()
    }

    fn page_up(&mut self) {
        self.scroll.page_up();
    }

    fn page_down(&mut self) {
        self.scroll.page_down();
        self.follow = self.is_at_tail();
    }

    fn jump_start(&mut self) {
        self.scroll.jump_start();
    }

    fn has_selection(&self) -> bool {
        TextViewport::has_selection(self)
    }

    fn selected_text(&self) -> Option<String> {
        TextViewport::selected_text(self)
    }

    fn clear_selection(&mut self) -> Outcome {
        TextViewport::clear_selection(self)
    }

    fn select_word_at(&mut self, pos: Position) -> Outcome {
        TextViewport::select_word_at(self, pos)
    }

    fn on_click(&mut self, pos: Position) -> Outcome {
        TextViewport::on_click(self, pos)
    }

    fn on_drag(&mut self, pos: Position) -> Outcome {
        TextViewport::on_drag(self, pos)
    }

    fn context_point(&self) -> Position {
        TextViewport::context_point(self)
    }

    fn keyboard_copy(&self) -> bool {
        true
    }

    fn caret(&self) -> Option<CellPos> {
        self.caret
    }

    fn caret_visible(&self) -> bool {
        self.caret_visible
    }

    fn screen_pos(&self, at: CellPos) -> Option<Position> {
        TextViewport::screen_pos(self, at)
    }

    fn draw(
        &self,
        area: Rect,
        buf: &mut Buffer,
        ctx: &mut RenderCtx,
        bg: Color,
        caret_allowed: bool,
    ) -> Option<Position> {
        let mut view = self.clone();
        view.caret_visible = self.caret_visible && caret_allowed;
        view.render(area, buf, ctx, bg);
        if view.follow && view.caret_visible {
            view.caret.and_then(|at| view.screen_pos(at))
        } else {
            None
        }
    }
}
