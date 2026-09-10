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

## Done

- [x] Obsidian-style local linking (see docs/superpowers/plans/2026-09-09-wikilinks.md).
