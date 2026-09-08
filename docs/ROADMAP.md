# mrkdup roadmap

Living list of where mrkdup goes next. Merged from the root `todo` file
(now removed) plus small follow-ups from the review of `main` at v0.2.0
(2026-09-08), when `cargo fmt --check`, `cargo clippy -- -D warnings`,
and `cargo test` (364 tests) were all green. Dated implementation plans
live in `docs/superpowers/plans/`.

The larger feature ideas (agent-launch, git awareness, checkbox index)
are deferred — not planned at this time.

## Small follow-ups (from the v0.2.0 review)

Correctness / edge cases:

- [x] Treat deleted-on-disk as a conflict, not a clean save.
  `disk_changed` returns `false` when either mtime is missing
  (`src/editor.rs`), so a dirty buffer whose file was deleted elsewhere
  saves without the usual conflict warning and silently recreates it.
  Decide: missing disk file should count as changed.
- [x] Document the trailing-newline normalization. `Editor::content`
  always appends a final newline (`src/editor.rs`), so a file without
  one gains one on first save. Likely intentional — say so in the
  `## Saving model` README section.
- [x] Reject trailing CLI args. `main` only inspects `args().nth(1)`
  (`src/main.rs`), so `mrkdup dir extra` silently ignores `extra` and
  `--help` after a directory is treated as a path.

Hardening (not defects):

- [x] Harden `atomic_write` (`src/fsutil.rs`): fixed
  `.NAME.mrkdup-tmp` name collides on concurrent saves of the same path
  and litters a dotfile after a crash; no directory fsync after rename,
  so the rename itself can be lost on some filesystems after a crash.
- [x] Replace direct tab indexing with a graceful error. `ed()` /
  `ed_ref()` index `self.tabs[self.active]` (`src/app.rs`) and panic if
  no file is open; every current caller guards first, but a status-bar
  error would fit the "never crash on bad input" ethos better than a
  future missed guard. (The `expect("a file is open")` nearby is
  `#[cfg(test)]` only — fine as is.)

Docs drift:

- [x] Fix stale `theme_name` doc comment (`src/config.rs`): says
  "(`default`, `light`, `mono`)" but there are five builtins
  (`firmitas`, `tokyonight` included). README is already correct.
- [x] Clarify the README config example. The `ini` sample shows
  non-default values (`autosave_seconds = 10`, `cursor_shape = block`,
  `cursor_blink = off`, `cursor_color = orange`) directly above the
  defaults table (`2` / `default` / `on` / `default`). Add a comment
  that these are example values, not defaults.
