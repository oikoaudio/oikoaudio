# Releases

Wow, Weft and Inton share one version, set in `[workspace.package].version` in the root `Cargo.toml`. Their manifests inherit it with `version.workspace = true`, and the release tooling rejects a plugin with a different version. Pushing one `v<version>` tag builds all three plugins together on each platform. The workflow then publishes a separate release for each product, with the tags and Linux archive names below.

| Plugin | Published release tag | Linux archive |
| --- | --- | --- |
| Wow | `wow/v<version>` | `wow-linux-x86_64.tar.gz` |
| Weft | `weft/v<version>` | `weft-linux-x86_64.tar.gz` |
| Inton | `inton/v<version>` | `inton-linux-x86_64.zip` |

Each product also gets `<product>-windows-x86_64.zip` and `<product>-macos-universal.zip`. The macOS bundles contain both arm64 and x86_64 binaries. Every archive contains the CLAP and VST3 bundles. Inton's archives also contain its MTS runtime and installation files. No archive contains an AUv2 build.

The shared build tag must match the workspace version exactly. During publication, the workflow creates the product tags at the same commit and never moves an existing tag. A tag that contains a hyphen creates a prerelease.

## What runs when

| Event | Result |
| --- | --- |
| Ordinary branch push | No workflow runs |
| Pull request | Workspace formatting, Clippy, dependency checks, and tests on Linux, macOS, and Windows |
| Shared `v<version>` tag push | Tests and bundles all products once per platform, then publishes each product's archives after every platform succeeds and starts the website deployment when configured |
| Product tag push | No build runs |
| Manual workspace check | Runs the same checks as a pull request |
| Manual release workflow | Builds downloadable test archives without publishing, including when run against a tag |

The manual release workflow builds for all platforms or for one of Linux, Windows or macOS. The archives appear as the workflow run's artifacts, grouped by platform. These workflows need a GitHub remote with Actions enabled. Local builds use `cargo xtask` as the root README describes.

## Preparing a release

Update the workspace version in the root `Cargo.toml` and update the lockfile. Record changes and compatibility warnings in the release notes of all three products. Test the bundles in a host. Commit those changes, then create and push the matching `v<version>` tag. Do not push the three product tags yourself. They do not start builds, and the workflow creates them.

Builds use the committed lockfile. The workflow runs the release-tooling tests and the notice-byte checks before it compiles anything. Each platform job tests the plugin packages and the extra test packages listed in `products.toml` in one `cargo test` run, then builds all three plugins with one `cargo xtask` bundle command. The full workspace check also tests each plugin on its own, which catches failures that feature unification hides. The build script checks that every macOS product bundle contains both architectures.

Before it changes anything on GitHub, the publication job checks all nine archives and the existing product tags. Each product's release stays a draft until all its uploads finish. A rerun of a failed publication job finishes the drafts and skips products already published with the exact same revision and asset checksums. The workflow never overwrites a published release, so changed assets need a new version. If only publication failed, rerun the publication job and leave the successful platform jobs alone.

CI does not replace host testing. GUI behavior and host-specific automation or modulation still need testing in a host. The workflows do not sign builds with a developer certificate and do not notarize them. The release tooling for the combined workflow has local tests, but the workflow has not yet completed a platform run on GitHub.

## Website distribution

The website is a separate repository. Its deploy workflow downloads the newest published release of each product, verifies every archive against the release's `checksums.txt` and `release.json`, and serves them under stable `downloads/<product>/latest/` URLs.

After publication, the release workflow starts that deployment if the `WEBSITE_UPDATE_TOKEN` secret holds a token with Actions write access to `oikoaudio/web`. Without the secret, the job only prints a notice. In that case, start the website's deploy workflow by hand once the website's release notes and manuals are ready.

## Shared release inputs

`products.toml` lists each hosted product's Cargo manifest, additional test packages, Linux archive format, release notes file and optional packaging script. The shared version comes from the root Cargo manifest. The bundle names come from `bundler.toml`. `.github/workflows/release.yml` runs `scripts/workspace_release.py`, which calls the product packaging functions in `scripts/release.py`. Inton also has its own packaging script, `plugins/inton/scripts/package.py`, which adds the MTS runtime.

The local entry points use the same configuration as CI:

```sh
python3 scripts/workspace_release.py test
python3 scripts/workspace_release.py build --platform Linux
python3 scripts/workspace_release.py package --platform Linux
```

Use `--platform macOS` for a universal build or `--platform Windows` for Windows. The `package` command writes one archive per product to `dist/`. To work on a single product, use `scripts/release.py`:

```sh
python3 scripts/release.py test wow
python3 scripts/release.py build wow --platform Linux
python3 scripts/release.py package wow --platform Linux --output dist/wow-linux-x86_64.tar.gz
```

Each archive contains its product's license, dependency inventory and third-party notices. Packaging copies only that product's CLAP and VST3 bundles, so other products and AU builds in the workspace bundle directory never end up in the archive. On macOS, packaging uses `ditto`, which keeps bundle metadata and symlinks.
