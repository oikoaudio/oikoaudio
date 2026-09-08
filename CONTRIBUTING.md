# Contributing

Read `docs/engineering-principles.md` and `docs/upstream-patches.md`. Put reusable mechanisms in `crates/` and product behavior in `plugins/`. Preserve plugin IDs, parameter IDs, serialized state, and timing unless the task explicitly changes their contracts. Product-specific native dependencies stay with their consumers. Add product directories when they contain a concrete product; do not create placeholder folders. Run commands from the repository root and use its lockfile, dependency patches, and `xtask`.

Declare `package.metadata.oiko.role` in each package manifest: `core`, `integration`, `ui`, `host`, `plugin`, or `tool`. Set `product` to its `products.toml` key for product-specific packages. Every direct or transitive consumer of an integration crate lists that package under `integrations`; shared UI and host crates cannot depend on integrations. `python3 scripts/check_workspace.py --offline` verifies all members and their normal/build dependencies with all features enabled, without compiling. Register hosted plugins in `products.toml` as well.

Change shared code and its consumers in the same commit when they depend on each other. Run `python3 scripts/check_workspace.py --test`, including individual plugin tests that catch differences hidden by workspace feature unification. Add regression tests for bugs and thread or lifecycle contracts. Record any untested native-host behavior. See [testing guidance](docs/testing.md) for product-specific native checks.

The workspace libraries are private implementation details with consumers maintained here. When moving or changing an internal Rust API, update its consumers directly rather than preserving old paths with compatibility re-exports or wrappers. Keep compatibility for distributed plugin identities, parameter contracts and saved-project formats; private Rust module paths do not carry that obligation.

Maintain upstream fixes only in `vendor/`. Preserve licenses, document each change, and refresh its exact patch artifact and reviewed checksums. Test the affected consumers before committing. A local commit does not imply permission to push, publish, or submit upstream changes.

Keep Markdown prose on one source line per paragraph or list item. Let the editor wrap it. Preserve intentional paragraph breaks, lists, tables, and code blocks.

Keep Rust test bodies in separate files, using private test modules for internal behavior and integration tests for crate boundaries. Group larger suites by responsibility. Test behavior, numerical results, lifecycle obligations, and persisted contracts. Do not pin UI wording, fixed widget positions, branding/version strings, or factory configuration values in assertions. Use widget identities to drive interactions and assert the resulting state. Coordinate-to-value calculations and serialization round-trips are behavior; repeating a declaration is not. Leave vendored upstream test organization unchanged.

Keep development research, experiments and detailed validation records in the ignored `.scratch/` directory. Product manuals should contain brief descriptions of user-facing behavior and controls, without links to private development notes.

Use Conventional Commits for new commit messages: `type(scope): description`, with a concise imperative description and a scope when useful. Use `feat` for new behavior, `fix` for corrections, `refactor` for restructuring, `docs` for documentation, `test` for tests, `ci` for workflows, `build` for build tooling, and `chore` for maintenance such as dependency upgrades (`chore(deps)`). Mark breaking changes with `!` before the colon and explain the compatibility impact in a `BREAKING CHANGE:` footer. Commit types do not publish releases; product releases remain explicitly controlled by release tags.

Keep original Oiko source MIT licensed and preserve third-party notices. Do not copy or vendor GPL-licensed source. Prefer published Rust dependencies unless a documented framework fix requires vendoring.
