# Fork maintenance map

This fork is `pupontech/Handy-Dictionary`; upstream is `cjpais/Handy`. Keep the
fork identifier `com.pupontech.handy.dictionary`, updater endpoint/public key,
portable-installer URL, and unsigned-platform build policy isolated from upstream.

## Intentional deltas

- Dictionary UI and import/export: `src/components/settings/dictionary/**`,
  sidebar/settings exports/store wiring, and `settings.dictionary.*` locale keys.
- Deterministic replacement engine: `src-tauri/src/audio_toolkit/dictionary.rs`,
  `AppSettings.dictionary_replacements`, its settings command, and final-output
  wiring. Never rewrite the upstream Custom Words fuzzy matcher.
- Direct GitHub sync: `src-tauri/dictionary-sync/**`,
  `src-tauri/src/dictionary_sync.rs`, the native command/service registration in
  `src-tauri/src/lib.rs`, dictionary-safe persistence in `settings.rs` and the two
  dictionary commands in `shortcut/mod.rs`, plus the sync UI and guides.
- Sync build/test support: the path dependency and lockfiles, Specta's JSON-value
  feature, `.github/workflows/dictionary-sync.yml`, and the two **public** GitHub
  App identity variables passed to Rust compilation in `build.yml`.
- Fork build/release identity in Tauri config and workflows; signed updater
  artifacts still use the fork key even though platform binaries are unsigned.

The [user sync guide](GITHUB-SYNC.md) is embedded directly into the app. The
[maintainer guide](GITHUB-SYNC-MAINTAINERS.md) defines GitHub App setup,
credential/privacy boundaries, failure ordering, and real-account release gates.

## Upstream updates

Inspect `git worktree list`, status, remotes, and open fork PRs first. Do not reuse
a dirty checkout or absorb another session's files. Fetch origin/upstream, create
an isolated worktree from fresh `origin/main`, and merge upstream with a merge
commit. Review conflicts by function, preserving Dictionary settings and the
shared dictionary-safe persistence path rather than accepting an entire side.

Use `gh ... -R pupontech/Handy-Dictionary` explicitly. Run repository lint,
formatting, frontend build, translations, model-language coverage, Playwright,
full Cargo tests, and the standalone sync library's tests/Clippy. Then open a fork
PR and require the seven-target PR Test Build before merging workflow/build
changes. Never merge, retag, or publish an installer merely because Linux unit
tests passed. Owner-live Windows dictation and two-device GitHub acceptance are
separate gates.

Keep the upstream-derived version plus `-dictionary` release convention; this
feature does not resolve or supersede a separate open version-correction PR.
