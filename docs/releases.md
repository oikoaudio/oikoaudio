# Releases

Each plugin has its own version and release tag. A tag builds that plugin's CLAP and VST3 bundles for Linux x86_64, Windows x86_64, and universal macOS, containing both arm64 and x86_64 binaries. Inton's archives also include its MTS runtime and installation files. AUv2 is not included.

| Plugin | Version source | Release tag |
| --- | --- | --- |
| WoW | `plugins/wow/crates/wow-plugin/Cargo.toml` | `wow/v<version>` |
| Weft | `plugins/weft/crates/spectral-plugin/Cargo.toml` | `weft/v<version>` |
| Inton | `plugins/inton/Cargo.toml` | `inton/v<version>` |

A pushed release tag must match the selected package version exactly. The workflow checks this before building. For example, version `0.4.0-beta.1` of WoW requires `wow/v0.4.0-beta.1`. Tags containing a hyphen create prereleases.

## What runs when

| Event | Result |
| --- | --- |
| Ordinary branch push | No workflow runs |
| Pull request | Workspace formatting, Clippy, dependency checks, and tests on Linux, macOS, and Windows |
| Product release tag push | Tests and bundles that product on all three platforms, then publishes its archives after every platform succeeds |
| Manual workspace check | Runs the same checks as a pull request |
| Manual release workflow | Builds downloadable test archives without publishing a release, including when run against a tag |

Run each product's release workflow manually to get test archives for all three plugins. Weft also has a Windows-only manual option. Downloads appear under the workflow run's artifacts. These workflows require a GitHub remote with Actions enabled; local builds use `cargo xtask` as described in the root README.

## Preparing a release

Update the product version and root lockfile, record the changes in the product's release notes, and test the bundles in a host. Commit those changes before creating the matching tag. Push only that tag when ready to publish. To release all three plugins, push each product's tag separately.

Builds use the committed lockfile. The common build script verifies that both macOS architectures are present in each bundle. The publishing job waits for all platform builds and uploads their archives to the matching GitHub release. A rerun can replace uploads in an unpublished draft. Published releases are never overwritten; use a new version for changed assets. The website update can be retried separately.

CI does not replace host testing. The workflows do not provide developer signing or notarization. GUI behavior and host-specific automation or modulation still need host testing. The initial monorepo workflows have been checked locally but have not yet run on GitHub.

## Shared release inputs

`products.toml` lists each hosted product's Cargo manifest, additional test packages, archive format and release notes or packaging hook. Versions come from the Cargo manifests and bundle names come from `bundler.toml`. The product tag workflows call `product-release.yml`; adding a product does not require copying the platform build and publication jobs. Inton retains its own runtime packaging script.

The local entry points use the same configuration as CI:

```sh
python3 scripts/release.py test wow
python3 scripts/release.py build wow --platform Linux
python3 scripts/release.py package wow --platform Linux --output dist/wow-linux-x86_64.tar.gz
```

Use `--platform macOS` for a universal build or `--platform Windows` for Windows. Use the root release script for every product. Archives include the selected product's license, dependency inventory, and third-party notices. They select only the requested product's CLAP and VST3 bundles, so a populated workspace bundle directory cannot leak other products or suspended AU outputs into an archive. macOS packaging uses `ditto` to preserve bundle metadata and symlinks.
