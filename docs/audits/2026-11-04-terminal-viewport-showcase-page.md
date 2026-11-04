# Terminal viewport showcase page is not in the sidebar (2026-11-04)

Source: P9.3, step B8-1 (`TerminalViewport`, BX26).

## The page is hidden

The page `terminalviewport` sits in `HIDDEN_ENTRIES` (`src/bin/showcase/app.rs`),
beside `terminalpane` and `fixedcontrols`. A `NAV_ENTRIES` row changes the
`Components 54` count and the sidebar of every showcase baseline, and
approved baselines must not change (D27).

Cost: the page opens only with `--page terminalviewport`, and the `/` filter
does not find it. It has six baselines: `showcase_page_terminalviewport`,
`showcase_terminal_viewport_focused`, `showcase_terminal_viewport_scrolled`,
`showcase_terminal_viewport_selection`, `showcase_terminal_viewport_fullscreen`
and `showcase_terminal_viewport_states`.

Next step: at the next intended showcase re-baseline, move the three hidden
entries to `NAV_ENTRIES` and accept every showcase baseline that shows the
sidebar in the same change, as recorded for `terminalpane` in
[2026-11-02-showcase-nav-and-listdetail-page.md](2026-11-02-showcase-nav-and-listdetail-page.md).

## The page does not copy to the clipboard of the host

Drag release and a double click call `Cx::copy`, which reaches the shell
clipboard (OSC 52 in `Host` mode) like every other showcase page. The
native-clipboard wording (`Copied` only after a native write) belongs to the
Capsule (B2c, B8b), not to this page.

## No coverage declaration file

As recorded for B4-0: `src/bin/showcase/coverage.rs` does not exist in this
tree. The new public API has its lines in `src/catalog.rs`, and the
`ViewportOps` allowlist line is deleted because the page now draws, scrolls,
selects and copies a viewport through that trait.
