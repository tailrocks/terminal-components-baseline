# Dependencies

Keep dependencies on their latest releases (crates, the Rust
toolchain, Tuiscotti, velnor-actions); upgrade at least monthly. A
version held below latest is recorded here with its blocker. Never
vendor or patch an upstream crate to get around a pin.

| Dependency | Pin | Notes |
| --- | --- | --- |
| Rust | 1.99.1 (`rust-toolchain.toml`, `mise.toml`) | Latest stable at last review. |
| `termpane` | git rev `0707acca3cbf3333d4c1b0e6d88b7b69169233dc` | The `terminal` feature's emulator (`junie_tui::terminal`), replacing `vt100`. Pinned to a git commit, not a crates.io release: the commit is termpane `main` and holds `EraseDisplayPolicy` (termpane PR #35, merged as f1c2dfde), which `TerminalState` sets to `Discard` so `ESC[2J` no longer moves the visible rows into history, as `vt100` behaved. 0.1.0 has no such switch. The pin becomes `=0.1.1` when 0.1.1 is published on crates.io (the operator says when). Default features only; `process` and `pty` spawn local PTYs and stay off. Brings `vte`, `compact_str` 0.10 and `smallvec`. |
| `tuiscotti-tui-test` (`tui_test`) | `=0.2.1` | Live PTY driver for functional tests. |
| `tuiscotti` (`tuiscotti-latest`) | git rev `d8d54155947f6303a41dfd6079b53c0abc16f29a` | Offline `FrozenStore` visual gate; bump to newest `main` rev deliberately when it moves. |
| velnor-actions | 0.1.72 (`mise.toml`) | Generates CI from `.velnor/config.toml`; see [CI](ci.md). |
| mr-boxington (`mbx`) | 1.29.1 (`mise.toml`, `postinstall = "mbx setup --yes"`) | Cargo wrapper via mise (`[wrappers.cargo]`, `mr_boxington = true` on the rust tool); CI uses the velnor `mbx` compile driver. |

Reviewed 2026-11-04. Testing-stack rationale is in
[functional testing](functional-testing.md).
