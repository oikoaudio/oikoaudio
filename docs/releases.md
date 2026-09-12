# Releases

Wow, Weft and Inton share one version from `[workspace.package].version` in the root `Cargo.toml`. Their manifests inherit it with `version.workspace = true`; release tooling rejects a plugin version that differs. Push one `v<version>` tag to build all three plugins together on each platform. The workflow publishes separate product releases and keeps their existing archive names.

| Plugin | Published release tag | Linux archive |
| --- | --- | --- |
| Wow | `wow/v<version>` | `wow-linux-x86_64.tar.gz` |
| Weft | `weft/v<version>` | `weft-linux-x86_64.tar.gz` |
| Inton | `inton/v<version>` | `inton-linux-x86_64.zip` |

Every product also gets `<product>-windows-x86_64.zip` and `<product>-macos-universal.zip`. The macOS bundles contain both arm64 and x86_64 binaries. Archives include CLAP and VST3; Inton's also include its MTS runtime and installation files. AUv2 is not included.

The shared build tag must match the workspace version exactly. Product tags are created at the same commit during publication, without moving existing tags. Tags containing a hyphen create prereleases.

## What runs when

| Event | Result |
| --- | --- |
| Ordinary branch push | No workflow runs |
| Pull request | Workspace formatting, Clippy, dependency checks, and tests on Linux, macOS, and Windows |
| Shared `v<version>` tag push | Tests and bundles all products once per platform, then publishes each product's archives after every platform succeeds |
| Product tag push | No build runs |
| Manual workspace check | Runs the same checks as a pull request |
| Manual release workflow | Builds downloadable test archives without publishing, including when run against a tag |

The manual release workflow can select all platforms or just Linux, Windows or macOS. Downloads appear under the workflow run's artifacts, grouped by platform. These workflows require a GitHub remote with Actions enabled; local builds use `cargo xtask` as described in the root README.

## Preparing a release

Update the root workspace version and lockfile, record changes and compatibility warnings in all three products' release notes, and test the bundles in a host. Commit those changes before creating and pushing the matching `v<version>` tag. Do not push three product tags to start builds.

The published `0.5.0-beta.1` releases used the previous product-tag workflows. Leave those tags and assets unchanged. Do not add a shared `v0.5.0-beta.1` tag to rerun them; use the consolidated workflow for the next version.

Builds use the committed lockfile. Release-tooling tests and notice-byte checks run before compilation. Each platform tests the selected plugin and core packages together, then builds all three plugins in one bundling invocation. The full workspace check still tests plugins individually to catch differences hidden by feature unification. The build script verifies both macOS architectures in every product bundle.

Publication checks all nine archives and existing product tags before making remote changes. Each product's uploads stay in a draft until complete. Rerunning a failed publication job can finish drafts and skip products already published with the exact same revision and asset checksums. Published releases are never overwritten. Use a new version for changed assets. If only publication failed, rerun that job rather than rebuilding successful platform jobs.

CI does not replace host testing. The workflows do not provide developer signing or notarization. GUI behavior and host-specific automation or modulation still need host testing. The consolidated workflow has local tooling tests; its first GitHub platform run is still pending.

## Website distribution

The website is a separate repository. Its current deploy workflow builds the checked-in download files and metadata; it does not fetch new plugin releases. Dispatching a website build alone does not update those downloads.

Plugin release jobs do not dispatch that website workflow. Prepare and review the website's release notes, manuals and download assets separately. While this source repository is private, anonymous visitors cannot download its GitHub release assets. Keep the repository private until its publication review is complete; do not replace working website downloads with inaccessible private-repository links.

## Shared release inputs

`products.toml` lists each hosted product's Cargo manifest, additional test packages, archive format and release notes or packaging hook. The shared version comes from the root Cargo manifest and bundle names come from `bundler.toml`. `.github/workflows/release.yml` runs `scripts/workspace_release.py`, which uses the existing product packaging functions in `scripts/release.py`. Inton retains its runtime packaging script.

The local entry points use the same configuration as CI:

```sh
python3 scripts/workspace_release.py test
python3 scripts/workspace_release.py build --platform Linux
python3 scripts/workspace_release.py package --platform Linux
```

Use `--platform macOS` for a universal build or `--platform Windows` for Windows. Packaging writes separate product archives to `dist/`. For work on a single product, the existing commands remain available:

```sh
python3 scripts/release.py test wow
python3 scripts/release.py build wow --platform Linux
python3 scripts/release.py package wow --platform Linux --output dist/wow-linux-x86_64.tar.gz
```

Archives include the selected product's license, dependency inventory and third-party notices. They select only that product's CLAP and VST3 bundles, so a populated workspace bundle directory cannot leak other products or suspended AU outputs into an archive. macOS packaging uses `ditto` to preserve bundle metadata and symlinks.
