# Contributing

Read `docs/engineering-principles.md` and `docs/upstream-patches.md` first. Put reusable code in `crates/` and product behavior in `plugins/`. Keep plugin IDs, parameter IDs, serialized state and timing unchanged unless the task is to change them. A product-specific native dependency belongs only to the products that use it. Add a product directory only when it contains a concrete product. Do not create placeholder folders. Run commands from the repository root and use its lockfile, dependency patches and `xtask`.

Declare `package.metadata.oiko.role` in each package manifest. The role is one of `core`, `integration`, `ui`, `host`, `plugin` or `tool`. For a product-specific package, set `product` to the product's key in `products.toml`. Every direct or transitive consumer of an integration crate lists that crate under `integrations`. Shared UI and host crates cannot depend on integrations. `python3 scripts/check_workspace.py --offline` checks these declarations for every workspace member without compiling. It inspects normal and build dependencies with all features enabled. Also register each hosted plugin in `products.toml`.

When shared code and its consumers depend on each other's changes, commit them together. Run `python3 scripts/check_workspace.py --test`. It also tests each plugin on its own, which catches failures that workspace feature unification hides. Add regression tests for bugs and for thread and lifecycle contracts. Record any native-host behavior you did not test. [docs/testing.md](docs/testing.md) lists the native checks for each product.

The workspace libraries are private, and all their consumers live in this repository. When you move or change an internal Rust API, update its consumers. Do not keep old paths working with compatibility re-exports or wrappers. Compatibility applies to released plugin identities, parameter contracts and saved-project formats. It does not apply to private Rust module paths.

Keep upstream fixes only in `vendor/`. Keep the upstream licenses, document each change in `docs/upstream-patches.md`, and regenerate its patch artifact in `patches/` and the reviewed checksums in `vendor-checksums.json`. Test the affected consumers before committing. A local commit does not give permission to push, publish or submit changes upstream.

Write each Markdown paragraph or list item on one source line and let the editor wrap it. Keep paragraph breaks, lists, tables and code blocks where the author put them.

Put Rust tests in separate files. Use private test modules for internal behavior and integration tests for crate boundaries. Split larger suites into files by the area they test. Test behavior, numerical results, lifecycle obligations and persisted contracts. Assertions must not check UI wording, fixed widget positions, branding or version strings, or factory configuration values. Drive interactions through widget identities and assert the resulting state. Coordinate-to-value calculations and serialization round-trips count as behavior. A test that repeats a declaration does not. Keep the upstream test layout of vendored sources unchanged.

Keep development research, experiments and detailed validation records in `.scratch/`, which Git ignores. Product manuals briefly describe user-facing behavior and controls. They do not link to private development notes.

Write new commit messages as Conventional Commits in the form `type(scope): description`. Use a short imperative description, and add a scope when it helps. Use `feat` for new behavior, `fix` for corrections, `refactor` for restructuring, `docs` for documentation, `test` for tests, `ci` for workflows, `build` for build tooling, and `chore` for maintenance such as dependency upgrades (`chore(deps)`). Mark a breaking change with `!` before the colon and explain the compatibility impact in a `BREAKING CHANGE:` footer. Commit types do not trigger releases. Only a pushed `v<version>` release tag starts a release.

Original Oiko source is licensed under MIT OR Apache-2.0. Keep third-party notices intact. Unless you state otherwise, a contribution you intentionally submit for inclusion, as defined in the Apache License 2.0, is dual licensed under the same terms, without additional terms or conditions. Do not copy or vendor GPL-licensed source. Use published Rust dependencies. Vendor a framework only when a fix documented in `docs/upstream-patches.md` requires it.

## Pull requests

1. Branch: create one branch per feature or fix, starting from `main`.
2. Scope: keep each pull request to one logical change. Split larger work into several pull requests, because each one becomes a single commit on `main`.
3. Tests: include tests for the change as described above, and record any native-host behavior you did not test.
4. Commits: commit on your branch however suits you. The branch is squashed when it merges, so there is no need to rewrite its history.
5. Title and description: write the title as a Conventional Commit, because it becomes the commit subject. The description becomes the commit body, so explain what changed, why, and any compatibility impact.
6. Status checks: the workspace checks on Linux, macOS and Windows must pass. For a first-time contributor, they start once a maintainer approves the run.
7. Review: a code owner listed in `.github/CODEOWNERS` must approve the pull request. New commits dismiss an earlier approval, and every review conversation must be resolved.
8. Up to date: if `main` has moved on, click `Update branch`. Either option works, because the branch is squashed.
9. Merge: once the pull request is approved, up to date and green, a maintainer merges it with `Squash and merge`.
