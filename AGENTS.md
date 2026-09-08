# Working on mrkdup (agent guide)

mrkdup is a small Rust TUI markdown editor: a `ratatui` + `crossterm` app
over `ratatui-textarea`, five dependencies, no async, no network. Read
`CONTRIBUTING.md` first — its ground rules (plain files are the only
state, never lose user data, modeless keys, focused modules, colors only
in `src/theme.rs`) are binding here too.

## Always update the README

**Any change that alters observable behavior must update `README.md` in
the same commit.** The README is the only user-facing documentation, so a
change that lands without it is incomplete — treat a stale README as a
failing check, not a follow-up task.

That covers a new or changed key or mouse action (the Keys / Mouse
tables), a new config option or a changed default, range, or meaning (the
`## Configuration` prose, the `ini` example, and the options table), a
new theme slot or builtin theme (the settable-keys list under
`### Themes`), a new CLI flag (`## Use`), new syntax highlighting or
markdown styling (the intro paragraphs), a change to saving, autosave, or
conflict behavior (`## Saving model`), and anything that changes how
mrkdup is installed.

Before you call a change done, re-read the sections your diff touches and
check the details still match the code — the tables carry defaults,
ranges, and exact key names that drift silently. The same applies to
`npm/README.md` when the install story changes.

## Layout

- `src/<module>.rs` with tests beside it in `src/<module>/tests.rs`,
  declared `#[cfg(test)] mod tests;` at the bottom of the module.
- Logic goes in a testable module; `src/ui.rs` and `src/main.rs` stay
  thin glue. `src/app.rs` dispatches keys and mouse events.
- Design docs and implementation plans live in `docs/superpowers/`.

## Checks

CI (`.github/workflows/ci.yml`) runs these on Linux, macOS, and Windows:

```sh
cargo fmt --check
cargo clippy -- -D warnings
cargo test
```

Run all three before claiming a change works. Tests use temp directories
and synthetic key events — no terminal needed.

## Releasing

Version lives in three places that must agree: `Cargo.toml`,
`Cargo.lock`, and `npm/package.json`. To cut a release:

1. Bump all three (`cargo update -p mrkdup --offline` refreshes the lock).
2. Commit, then `git tag -a vX.Y.Z && git push origin vX.Y.Z`.
3. Wait for the Release workflow — it attaches one archive per target,
   named exactly as `npm/install.js` expects to download them.
4. `cargo publish`, then `cd npm && npm publish --access public`.
