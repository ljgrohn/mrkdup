# Contributing to mrkdup

Thanks for your interest! mrkdup is intentionally small, and contributions
that keep it that way are the most likely to land.

## Ground rules

- **Plain files are the only state.** mrkdup edits text files in place — no
  sidecar files, no databases, no per-project config. What's on disk is the
  truth.
- **Never lose user data.** All writes go through the atomic temp-file+rename
  path, and a dirty buffer must never silently clobber a file that changed on
  disk. Any change touching save, reload, or conflict logic needs tests
  proving those guarantees hold (see `src/editor.rs` for the pattern).
- **Modeless and predictable.** One focus toggle, no modes. New keybindings
  should be boring and guessable; anything vim-shaped belongs behind a future
  opt-in, not in the default map.
- **Keep modules focused.** `tree.rs`, `editor.rs`, `app.rs`, `ui.rs`, and
  `fsutil.rs` each have one job. Logic goes in a testable module; `ui.rs` and
  `main.rs` stay thin glue.
- **Colors live in `src/theme.rs`.** Don't add `Color::` literals in `ui.rs`,
  `render.rs`, or `highlight.rs` — add a `Theme` slot instead.

## Workflow

1. Fork and branch from `main`.
2. `cargo test` — add tests for tree, editor, or dispatch changes (the
   existing tests run against temp directories and synthetic key events, no
   terminal needed). Each module's tests live beside it in
   `src/<module>/tests.rs`, declared with `#[cfg(test)] mod tests;` at the
   bottom of `src/<module>.rs`; as a child module it can still reach the
   module's private items.
3. `cargo fmt` and `cargo clippy -- -D warnings` — CI enforces both.
4. Open a PR with a short description of the behavior change.

For anything bigger than a bug fix, opening an issue first will save you time.
