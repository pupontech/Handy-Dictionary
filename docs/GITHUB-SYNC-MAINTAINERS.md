# GitHub sync: maintainer setup and architecture

## Registration is a release prerequisite

The implementation cannot invent a registered GitHub App identity. Register **one public GitHub App** owned by the fork maintainer, so everyone can install it on their own private data repository. Do not ask every user to register an app, supply a PAT, clone a repository, or run a server.

This registration requires the maintainer's interactive GitHub approval. Source code and fixture tests alone do not prove that real authorization works. Do not publish this feature as ready until the registration and real two-computer acceptance below have been completed.

### GitHub App settings

In GitHub account/organization settings, open Developer settings → GitHub Apps → New GitHub App:

1. Choose an available app name; its public URL determines its **slug**. Link its homepage to this fork and its privacy/setup description to `docs/GITHUB-SYNC.md` in the published fork. Make the user-facing description explicit: private repository dictionary storage, no maintainer sync server, local OS token storage.
2. Set **Where can this GitHub App be installed? → Any account**. This makes the app public/installable, not users' repositories public.
3. Under **Repository permissions**, set **Contents → Read and write**. Metadata read is implicit. Leave other repository, organization, and account permissions at no access.
4. Do not subscribe to events. **Disable active webhooks**; there is no webhook receiver or maintainer backend.
5. Enable **Device Flow**. This implementation uses the documented GitHub App device-authorization flow. It displays GitHub's verification code, keeps the device secret native, and enforces GitHub's polling interval and slowdown response.
6. Leave **expiring user access tokens enabled** for the recommended least-persistence policy. GitHub normally expires them after eight hours. This client intentionally reauthorizes interactively instead of embedding a client secret to refresh them. If you change this setting, document the resulting trade-off and test the real behavior; do not imply automatic refresh exists.
7. Copy the app's **Client ID**, not its numeric App ID, and record its public **slug**. These two identifiers are public. **Never embed a client secret or generated private key.** Neither is needed for the initial device flow. If GitHub requires an administrative URL field, use a maintainer-owned HTTPS documentation/home URL; this client does not receive authorization callbacks at that URL.
8. Test installation with **Only select repositories**, on a disposable private data repository initialized with a README.

Device authorization has phishing risks. GitHub's current best practices prefer authorization code + PKCE where practical. The deliberately serverless device flow here avoids shipping an app secret; users must initiate Connect themselves and verify the exact app and code in their browser. Do not enable or reuse device flow to authorize unrelated services, and do not accept device codes through support messages.

Authoritative references:

- [GitHub App user tokens and device flow](https://docs.github.com/en/apps/creating-github-apps/authenticating-with-a-github-app/generating-a-user-access-token-for-a-github-app)
- [GitHub App security practices](https://docs.github.com/en/apps/creating-github-apps/about-creating-github-apps/best-practices-for-creating-a-github-app)
- [Repository Contents API](https://docs.github.com/en/rest/repos/contents)

### Supplying public configuration

For a distribution build, provide these environment variables **to the Rust compilation step**:

```text
HANDY_GITHUB_CLIENT_ID=<the registered GitHub App's public Client ID>
HANDY_GITHUB_APP_SLUG=<the registered app's public URL slug>
```

The Rust backend uses `option_env!`, so these are compiled into the binary. Setting an environment variable only when launching an already-built executable does not configure that executable. They are public identifiers, not GitHub Actions secrets; use ordinary repository variables when configuring CI. Verify the exact reusable build workflow receives them before building a release.

A test/unconfigured build also exposes **Public GitHub App configuration** in Dictionary → GitHub sync. Enter the same registered app's public Client ID and slug there before Connect. This is a way to exercise a test build without recompilation, not a request for each user to register their own application. Users must trust the supplied app identity. A missing configuration is shown explicitly; no fake ID or unrelated application's ID is substituted.

Do not ship an unconfigured build as a seamless public sync release. Do not confuse GitHub authentication used by the development machine/CI with application-user authorization. Never extract the developer's `gh` token for the app or for live sync tests.

## Files and boundaries

- `src-tauri/dictionary-sync`: small independently testable Rust library for schema validation, three-way merging, GitHub HTTPS calls, credential-store access, and local metadata/backups.
- `src-tauri/src/dictionary_sync.rs`: Tauri orchestration and native command registration. It keeps pending authorization/conflict state in native memory and serializes sync operations with a try-lock.
- `src-tauri/src/settings.rs`: dictionary-only patch and compare-and-swap, plus preservation of current dictionary fields in unrelated settings saves. The shared settings lock prevents settings writers and migration writes from racing dictionary updates. Existing Custom Words and Replacement Rules commands use this same patch path.
- `src/components/settings/dictionary/DictionarySync.tsx`: connection, repository selection, first-sync consent, manual sync, conflict preview, and the embedded user guide. No password/PAT input and no token returned through IPC.
- `docs/GITHUB-SYNC.md`: canonical detailed user guide, included in-app from this same Markdown source. Update it when behavior changes; do not maintain a separate stale in-app copy.

The feature deliberately does not change the fuzzy Custom Words matcher, deterministic replacement engine, transcription output pipeline, updater identity, or model storage.

## Persistence and merge contract

Only `customWords` and `replacements`, with the existing `handy-dictionary` schema/version envelope, are serialized remotely. The format is compatible with full-dictionary JSON export. Incoming sync files are stricter than forgiving imports: malformed/unknown fields, unsupported schema/version, duplicate case-insensitive entries, invalid spacing, empty rule sides, overlong words, and oversized payloads are rejected in full. There is a 10,000-entry/1,000,000-byte dictionary limit and bounded HTTP response decoding.

Local `dictionary-sync/state.json` contains the public Client ID/app slug, pinned repository owner/name/ID/branch, last-success time, and last-applied dictionary baseline. Unknown state fields/corrupt files are not silently ignored. Writes use a temporary file in the destination directory, flush/sync, and replacement. OS credential entries use service `com.pupontech.handy.dictionary.github`, account = public Client ID. Credential-store errors are sanitized and never trigger file-based storage.

First sync merges against an empty baseline and requires explicit consent. Subsequent syncs use a three-way baseline; absence of a baseline entry on a changed side is a deletion. Disconnect retains baseline/repository metadata. Selecting a different repository or branch resets baseline explicitly and creates a backup. Restoring/moving a portable data folder cannot restore an OS-account token; same-OS-account installations using the same Client ID share the credential entry.

Merge keeps current local order, appending previously unseen remote entries. Word identity and replacement source identity are case-insensitive; source-identity changes are delete+add. Same-key divergent edits or edit-vs-delete are conflicts. The two conflict choices apply only to conflicting entries, not whole-dictionary replacement. Native pending previews bind a resolution to the exact local snapshot, remote content, and remote SHA. Changed previews are rejected.

## Commit and failure ordering

1. Require first-sync consent when no baseline exists.
2. Load the token from the OS store and verify the repository is still private and has the expected numeric repository identity.
3. Snapshot/validate local dictionary and fetch/validate the remote file on the pinned branch.
4. Refuse a missing remote file after an established baseline. Never silently resurrect it.
5. Reconcile, or return a conflict preview without writing either dictionary.
6. Save a pre-sync local backup (retain twenty) before changing either side.
7. Refuse if local settings changed during preparation.
8. If necessary, PUT only `handy-dictionary.json`, using GitHub's existing blob SHA as compare-and-swap. Concurrent writes fail, never force-push.
9. Read the exact target back and compare its dictionary content with the intended upload.
10. Compare-and-swap the local dictionary, preserving unrelated latest settings; save the settings store synchronously.
11. Save the new baseline/time and notify the frontend to refresh.

No remote/local cross-machine transaction is possible. A remote commit can succeed before a later verification, local compare-and-swap, or disk operation fails. Report failure, retain a safe baseline, and reconcile again on the next explicit sync. Do not promise all-or-nothing distributed writes. A file revision cannot make a concurrent repository visibility change atomic; the code checks visibility immediately before fetching/writing, but the account owner is still responsible for keeping the repository private.

## Verification and acceptance

The library runs independently of GTK/speech-model builds:

```sh
cargo test --manifest-path src-tauri/dictionary-sync/Cargo.toml --locked
cargo clippy --manifest-path src-tauri/dictionary-sync/Cargo.toml --locked --all-targets -- -D warnings
```

Tests exercise a real local HTTP server using the real HTTP client; responses are explicit fixtures, not a claim of live GitHub acceptance. Production endpoints are fixed to `github.com`/`api.github.com`, redirects are disabled, and test endpoints are private to the test module. Error text is sanitized rather than forwarding GitHub response bodies.

Run the actual OS credential-store integration test separately on a Windows GitHub Actions runner. It writes only a synthetic marker under a process-specific test account, reopens the credential entry, verifies it, and deletes it:

```sh
cargo test --manifest-path src-tauri/dictionary-sync/Cargo.toml --locked credentials::tests::native_credential_store_roundtrip -- --ignored --exact
```

Require an actual executed-test count, not a zero-test successful exit. Do not run the mock credential-store test in the same process as the native test; the mock builder intentionally changes a process-global test setting.

Also run the fork's lint, formatting, TypeScript/frontend build, translation parity, Playwright, full app Cargo checks/tests, and PR Test Build matrix. Build/deploy identity remains unchanged by this feature; do not manufacture a new fork version just to sidestep the existing release/version PR.

### Required real-account acceptance before public release

- Register the actual public app and verify its displayed identity/permission screen.
- In a disposable **private** repository, use two real Windows installations/OS accounts (or the owner's two devices). Use distinct harmless dictionary entries, not production/sensitive terms.
- Complete install, device authorization, local credential save, repository selection, first-sync upload, and download from the second computer.
- Inspect the actual GitHub file/history; verify only dictionary data and a generic commit message were written. Verify no password, token, recordings, or unrelated settings appear in settings/export/state/backup files or ordinary sync logs. Do not disclose actual tokens in the evidence report.
- Demonstrate independent offline additions, deletion propagation, divergent replacement edits, edit-vs-delete conflict, explicit resolution, and a stale preview rejected after another device changes it.
- Exercise revocation/expiration, offline networking, repository permission removal, public-repository refusal, missing remote file, and locked/unavailable credential storage.
- Disconnect and verify the specific local credential entry is gone while local dictionary and remote data remain.
- Test reconnect without resurrecting deletions and portable-folder movement without moving authorization.

Separate fixture/unit tests, actual Windows credential-store tests, installer build proof, and real GitHub/two-device acceptance in the report. None substitutes for the others.
