## What and why

<!-- What changes, and the problem it solves. Link the issue: "Closes #123". -->

## How it was verified

<!-- Commands run and what you checked by hand (screenshots for UI changes). -->

## Checklist

- [ ] I have signed the [Contributor License Agreement](https://github.com/humcase/humcase/blob/master/CLA.md) (the bot asks on your first pull request)
- [ ] Title follows Conventional Commits (`feat:`, `fix:`, `docs:`, `refactor:`…)
- [ ] `cargo test`, `cargo clippy --all-targets -- -D warnings` and `cargo fmt --check` pass (Rust changes)
- [ ] `npm test`, `npm run lint -- --deny-warnings` and `npm run build` pass in `frontend/` (frontend changes)
- [ ] User-visible text goes through i18next, in English and French
- [ ] A new data-loss guard comes with a test that fails without it
- [ ] `docs/CHANGELOG.md` updated for user-visible changes
