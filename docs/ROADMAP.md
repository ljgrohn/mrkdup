# mrkdup roadmap

Living list of where mrkdup goes next. Merged from the root `todo` file
(now removed) plus small follow-ups from the review of `main` at v0.2.0
(2026-09-08), when `cargo fmt --check`, `cargo clippy -- -D warnings`,
and `cargo test` (364 tests) were all green. Dated implementation plans
live in `docs/superpowers/plans/`.

The larger feature ideas (agent-launch, git awareness, checkbox index)
are deferred — not planned at this time.

## Proposed

- [ ] Share one `[text](url)` tokenizer between `src/highlight.rs` and
  `links::parse_md_link_at`, mirroring `links::wikilink_span` — they
  hand-roll the same bracket rules today and agree only by inspection.
- [ ] Count local `[text](path)` links as backlinks in
  `links::backlinks`, not just `[[wikilinks]]` — `Ctrl+O` follows both
  but `Ctrl+L` counts only wikilinks.
- [ ] Tell the user when a backlink scan hit
  `fuzzy::collect_candidates`' 5000-file cap; today the truncation is
  silent, so an over-cap vault can report "no links yet" when links
  exist.
- [ ] Treat a single-colon scheme as remote in `links::is_remote_url`
  (`tel:`, `obsidian:`): only `://` and `mailto:` are recognized today,
  so `Ctrl+O` on one reports `no file '<url>'` instead of saying it is
  not a local file.
- [ ] Fold case properly in `links::may_name`: it compares with
  `eq_ignore_ascii_case` and strips a case-sensitive `.md`, so on a
  case-insensitive disk a note named `X.MD` gets no backlink from
  `[[X]]`, and non-ASCII case never matches.
- [ ] Decide whether the link-follow create offer should survive typing
  (`src/ui.rs`): it reads `app.status`, which `App::handle_key` clears on
  every keypress, so the explanation vanishes as soon as the user edits
  the prefilled name.
- [ ] Stop `files::move_to` silently replacing a dangling symlink at the
  destination: its `target.exists()` check follows links, so the
  `fs::rename` overwrites it (pre-existing, found during the wikilinks
  review).
- [ ] Cover the untested branches left by the wikilinks review:
  `fsutil::canonical`'s failure fallback, `files::selected_dir`'s
  no-selection case, the `heading is beyond line 65535` status, and
  `follow_md_url`'s `no file '#'`. Tighten `links.rs`'s public surface
  while there — `normalize_lexical` and `percent_decode` are `pub` with
  no caller outside the module, and `MdLink`'s `start`/`end` are never
  read.

## Done

- [x] Obsidian-style local linking (see docs/superpowers/plans/2026-09-09-wikilinks.md).
