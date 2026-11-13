# Contributing: verify, test in isolation, deliver

Complements [AGENTS](../AGENTS.md) (rules). Maintainers keep dated verification lessons in private notes.

## Contributor License Agreement

Before your first pull request is merged, you sign the [Contributor License Agreement](../CLA.md) once: the CLA Assistant bot comments on the pull request and you reply as it asks. You keep the copyright of your work; the agreement lets the maintainer license Humcase as a whole, and every contribution stays available under the AGPL-3.0. If part of your contribution was produced with an AI tool, say so in the pull request and review it as your own work (section 6 of the agreement).

## Verification

```bash
cd src-tauri
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

The website, when `site/` changed: `npm run check-i18n --prefix site` and `npm run build --prefix site`.

**CI**: `ci.yml` runs on every pull request and every push to `master`; a PR is merged once it is green. Run the checks of AGENTS.md (Verification) locally for the code you touch before pushing. **Platforms**: CI runs on Windows (`windows-latest`), the reference platform. Linux is supported best-effort: `cargo test` and `cargo clippy --all-targets -- -D warnings` are expected to pass there too (see "Linux build" below to run them in Docker; natively, install the Tauri prerequisites `libgtk-3-dev libwebkit2gtk-4.1-dev libsoup-3.0-dev libjavascriptcoregtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev patchelf`). Points specific to Unix:
- the generic `ai_settings` tests (lock, quarantine, alert, migration from plaintext) use a fake encryption **compiled only in tests outside Windows** (`mod dpapi` under `cfg(all(not(windows), test))`); the real Unix sealing (`mod keyfile`, XChaCha20-Poly1305 under `ai-config.key`) has its own tests in `ai_settings_unix_tests.rs`. The real DPAPI protection is exercised only on Windows: running `cargo test` there remains necessary before a delivery touching `ai_settings`;
- `PidLiveness::Dead` is built by the Windows probe and the Linux `/proc` probe; the `allow(dead_code)` attribute targets only the other systems.

## Linux build

Docker Desktop (or Docker on Linux) is the only prerequisite; nothing runs in CI.

```bash
npm run build-linux              # AppImage + .deb + humcase binary in tools/linux/out/
bash tools/linux/build.sh check  # cargo fmt --check, clippy and cargo test on Linux
bash tools/linux/build.sh shell  # shell in the build container
bash tools/linux/build.sh smoke cli            # install the .deb in a throwaway container: CLI + MCP session
bash tools/linux/build.sh smoke ui "417,603"   # Xvfb + openbox, launch the AppImage, screenshots in out/screenshots/
```

`tools/linux/Dockerfile` is Ubuntu 22.04 (the AppImage runs on distributions at least as recent) with the Tauri system libraries, Node LTS, rustup, and Xvfb/openbox/ImageMagick/xdotool for screenshot tests. The repository is mounted read-only and copied inside the container; the Rust toolchain, cargo registry, `target/` and npm cache live in the Docker volumes `humcase-linux-cache` and `humcase-linux-npm`, so rebuilds are fast (`docker volume rm` them to start clean). `tools/linux/out/` is git-ignored. Test a bundle inside a container with an isolated `HOME`, never with real data.

```bash
cd frontend
npm test
npm run lint -- --deny-warnings
npm run build
```

`npm test` (Vitest + jsdom) covers data non-regression: typing during a slow save, switching notes during a dictation, AI provider failure, deletion undo, priority removed, write failure on a nonexistent note, etc. Every new data-loss safeguard comes with a test that **fails** if the safeguard is removed (counter-test).

Development mode: `npm run tauri dev` at the root. MCP server by hand: `cargo run -- mcp` in `src-tauri`.

## Interface texts (i18n)

No hard-coded visible text: `const { t } = useTranslation()` then `t('zone.key', { variable })`. Add the key in `frontend/src/i18n/locales/fr/<zone>.json` **and** `en/<zone>.json` (native English, terms from [GLOSSARY](GLOSSARY.md)); i18next plurals (`key_one` / `key_other` + `count`); dates through `Intl` with the current language. Outside a component: `i18n.t` (default import from `src/i18n`) called at display time, never in a constant evaluated at load. Never make a behavior depend on a translated text. On the Rust side: `crate::tr!("zone.key", name = value)`, keys in `engine.json` (fr and en); a test that checks French text runs it under `crate::i18n::with_language("fr", ...)`.

## Isolated test profile

To check notes through MCP and their display without touching the user database, the **debug** builds of the CLI, the MCP and the interface accept `HUMCASE_TEST_DATA_DIR`.

- First create an absolute folder containing the file `.humcase-isolated-profile`; an invalid path is refused.
- The profile holds the whole data folder: the SQLite database, its process lock, the backups (manual and safety archives, in the profile's `backups`), pasted images, AI keys and settings, window state and WebView storage.
- A release always ignores this variable and uses `%USERPROFILE%\.humcase`. Never use the user database as a test folder.
- Outside an isolated profile, the interface is single-instance: a second launch brings the open window to the front and exits (the MCP is not concerned).
- Stop a debug build by its process id: the debug binary is `src-tauri/target/debug/humcase.exe` and Windows compares process names without case, so `taskkill /IM humcase.exe` also closes the installed `Humcase.exe`.
- A large profile from a JSON file (projects, cases and sub-cases with their dates, due days and the agent that declared a result): `HUMCASE_SEED_FILE=profile.json HUMCASE_SEED_DIR=C:\tmp\profile cargo test seed_profile -- --ignored` (`src-tauri/src/profile_seed.rs`, format in its header). It writes through the application's own functions and refuses a folder that is not a new or marked test profile.
- Interface inspection: WebView2 + CDP (`WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9222`). Visual check at 720/1920/2560 px, two themes, narrow window.

Migration trials: on temporary databases or dedicated copies; make a safety copy of the real database before delivering a migration.

## Agent plugin

`plugins/humcase` holds the agent packaging: the Claude Code manifest (`.claude-plugin/plugin.json`, listed by the marketplace `.claude-plugin/marketplace.json` at the root), the Codex manifest (`.codex-plugin/plugin.json`) and the skills they share (`skills/<name>/SKILL.md`). Client setup: [AGENT_SETUP](AGENT_SETUP.md).

- **No `version`** in the plugin manifests nor in the marketplace entry. A pinned version keeps every installed copy on its cache until the string changes; without it, Claude Code takes the git commit as the version, so a skill change reaches users at their next plugin update, with no release. `claude plugin validate` warns about the missing version: that warning is expected.
- **Skills**: `name` equal to the folder, a `description` in the third person of at most 1,024 characters that says what the skill does and when to use it, a body under 500 lines, references one level deep. Tools are cited as `mcp__humcase__<tool>` with the CLI fallback `Humcase.exe call <tool> --args '<json>'`; vocabulary follows [GLOSSARY](GLOSSARY.md) (case, sub-case, nature, report, result, rework).
- **Check**: `npm run check-plugins` (also run by the CI) parses every manifest, refuses a `version`, checks the `${user_config.*}` references and the skills' front matter, length and links. With the Claude Code CLI, also run `claude plugin validate plugins/humcase` and `claude plugin validate .`.

## Delivery

1. Verifications above, then describe the change under `## [Unreleased]` in `docs/CHANGELOG.md`; a release renames that section to a dated `## [x.y.z] - date` entry. `npm run update-exe` takes the latest dated entry as the reference version and aligns `frontend/package.json`, `frontend/package-lock.json`, `src-tauri/Cargo.toml` and `src-tauri/tauri.conf.json` with it (`src-tauri/Cargo.lock` follows at the next cargo build).
2. Close `Humcase.exe` (interface and MCP servers that use it), then `npm run update-exe` at the root. The script builds the release, replaces `Humcase.exe`, checks the SHA256 fingerprint and fails explicitly (code 1) otherwise. It stops no process.
3. Check the new MCP server with a direct call or through the client used for the task, and only then resolve the Humcase note.
4. GitHub release: pushing a `v*` tag equal to the latest dated CHANGELOG version runs `release.yml` (version check, tests, build, release marked latest with the installer also named `Humcase-setup-x64.exe`, the file the website links to). Only maintainers push tags.
