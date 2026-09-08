# mrkdup roadmap

Living list of where mrkdup goes next. Merged from the root `todo` file
(now removed) plus small follow-ups from the review of `main` at v0.2.0
(2026-09-08), when `cargo fmt --check`, `cargo clippy -- -D warnings`,
and `cargo test` (364 tests) were all green. Dated implementation plans
live in `docs/superpowers/plans/`.

## Direction — decide first

- [ ] Decide the product thesis before building the trio
  The three features below are one animal, not three: agent-launch
  starts the work, git shows what it changed, the checkbox index steers
  it — mrkdup as the human's window into agent-driven work. Build order
  should be agent-launch → git status → checkbox index; each makes the
  next more useful and none adds a dependency. Worth being deliberate
  about whether that IS the direction, or whether mrkdup stays a
  general markdown editor that merely plays well with agents and we just
  cherry-pick the cheap wins (branch in status bar, tree colors).

## Features

- [ ] Launch an AI agent from inside the editor (suspend-and-exec)
  The pitch: mrkdup is already the place you are when you decide a
  document needs work. Today that decision means leaving — new pane,
  cd to the right folder, re-explain where you are. A hotkey that drops
  you into `claude` (or codex, grok, aider) already scoped to the open
  file's folder collapses that to one keystroke, and the round trip
  back is free: the agent edits the file on disk, mrkdup's existing
  external-change detection notices and reloads the buffer. The whole
  feature is a hole punched in the TUI, not a new subsystem.
  Shape: leave the alternate screen, disable raw mode, spawn the agent
  as a child sharing our stdio with cwd = the open file's folder (tree
  focus: the selected dir) and the path passed as context, wait, then
  restore the screen and re-check every tab against disk. Zero new
  deps, no async, no API keys, no network — the agent binary owns all
  of that. Configurable command in the config file so it isn't
  Claude-specific (`agent_command = claude`).
  Do NOT build the expensive version: an embedded chat pane means an
  async runtime, an HTTP client, streaming, and key management — it
  would roughly double the codebase and break the "thin wrapper around
  plain files, no network" thesis for maybe 10% more value.
  Open questions: does the agent get the file path, the folder, or
  both? What happens to unsaved buffers before we suspend (force an
  autosave — the agent must see what's on screen). Do we need a second
  binding for "agent on the whole tree root" vs "this file's folder"?

- [ ] Git awareness: branch in the status bar, file status in the tree
  The pitch: once an agent is editing files underneath you, the
  question you have every thirty seconds is "what just changed?" Right
  now the tree can't answer it — every file looks identical whether it
  was touched a second ago or never. Coloring modified/untracked/staged
  rows turns the tree into a live diff summary, which is the single
  highest-value pixel change available, and it costs nothing at rest.
  Shape: shell out to `git status --porcelain=v1 -z` and
  `git rev-parse --abbrev-ref HEAD` on the same ~2s cadence as the
  existing tree refresh; map paths onto tree rows; new theme keys
  (`git_modified`, `git_untracked`, `git_staged`). Branch name in the
  status bar next to the word count. Degrades silently to today's
  behavior outside a repo.
  Explicitly NOT taking a git library. `git2`/`gix` would dwarf the
  current five-dep tree and the binary size for something a subprocess
  does fine.
  Scope line to hold: this is LOCAL git. Remotes, PRs, review, and auth
  are a different application — the moment "GitHub" enters, so do
  tokens, HTTP, and rate limits.
  - [ ] Branch name in the status bar (trivial, do it first)
  - [ ] Porcelain status → tree row colors + theme keys
  - [ ] Worktree picker — the actually-interesting piece
    Agents work in worktrees. `git worktree list` in a popup that
    re-roots the tree at the chosen one is a small extension of the
    `+`/`-` re-root gesture that already exists, and it's how you'd
    watch an agent work a branch without touching your own checkout.
    Branch *switching* from inside the editor is the risky version
    (files change under open dirty tabs) — defer it; the conflict
    machinery is closer to ready than most editors' but it's still a
    sharp edge.

- [ ] Tree-wide checkbox index (a view, not a todo app)
  The pitch: checkboxes are already the lingua franca between you and
  an agent — it writes `- [ ]` next steps into a plan file, you tick
  them off. But they scatter across a dozen markdown files and there's
  no way to see them at once. A pane that scans every markdown file
  under the root for `- [ ]` / `- [x]`, lists them grouped by file, and
  jumps to the exact line on Enter turns the whole tree into one
  worklist without inventing a format, a database, or a sync protocol.
  Nothing new to teach an agent: it already writes markdown.
  Shape: reuse the `ignore` walker and the existing highlight
  tokenizer, which already recognizes checkbox lines. Read-only
  aggregation — no new storage, no state file. Ctrl+D on an indexed row
  toggles through to the real file. Cache the scan, refresh on the
  tree's existing cadence.
  The boundary that keeps this from being locdo: locdo owns ONE
  curated file with sections, archiving, and its own state machine.
  This owns ZERO files — it's a derived view over whatever markdown
  happens to be in the tree, the way the fuzzy finder is a view over
  filenames. If it ever grows its own file format or its own storage,
  it has become locdo and should be deleted.
  Open questions: filter to unchecked only, or show both dimmed? Does
  the index live in a popup (like the fuzzy finder) or a third pane?
  Cap on tree size before the scan gets expensive?

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
- [ ] Clarify the README config example. The `ini` sample shows
  non-default values (`autosave_seconds = 10`, `cursor_shape = block`,
  `cursor_blink = off`, `cursor_color = orange`) directly above the
  defaults table (`2` / `default` / `on` / `default`). Add a comment
  that these are example values, not defaults.
