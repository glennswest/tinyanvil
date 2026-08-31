# Changelog

## [v0.4.1] — 2026-08-31

### Added
- **feat:** Goldens keep their provenance: `<name>.delta.tar` (the package layer) and
  `<name>.meta` (base, repos, packages, built, delta size) beside the image, covered by
  SHA256SUMS. Enables re-forge after base updates and a future nested/slab form.
- **docs:** Promotion decision recorded — flat/self-contained is the default; `--nested`
  and `tinyanvil promote` arrive with stormblock slab CoW (#87).

## [v0.4.0] — 2026-08-31

### Added
- **feat:** `--repo` — enable extra repos via their release RPM (aliases rpmfusion-free/
  rpmfusion-nonfree, or any release-RPM URL); resolution then uses the image's own repo
  config. EPEL is refused with an explanation (EL-only; built from Fedora).
- **feat:** Delta-layer size discipline: strip usr/share/{locale,doc,man,info} additions
  from the upper layer; report delta size per build.
- **docs:** Roadmap: GUI ships as a stormconsole plugin on StormCOS driving `tinyanvil serve`.

## [v0.3.0] — 2026-08-31

### Changed
- **BREAKING:** `anvil.sh` replaced by the `tinyanvil` Rust binary (std-only, zero crate
  deps, static musl): same base/build/list verbs, RAII mount+loop cleanup on every failure
  path, streamed dnf/stormblock output. Version now lives in Cargo.toml + VERSION.

## [v0.2.0] — 2026-08-31

### Changed
- **BREAKING:** Project renamed tinyforge -> tinyanvil ("forge" reads as counterfeiting;
  the anvil is pure blacksmithing). Script is now `anvil.sh`, store is `/build/tinyanvil`,
  env var TINYANVIL_STORE. GitHub redirects the old repo URL.

## [v0.1.0] — 2026-08-31

### Added
- **feat:** Initial tinyanvil — anvil.sh with `base` (tinystorm raw -> base tar + sealed
  golden via stormblock), `build` (overlayfs + host-side dnf5 -> delta layer -> layered
  golden with license manifest and SHA256SUMS entry), `list`.
- **fix:** Tar layers carry only the security.capability xattr — selinux/acl attrs fail
  stormblock's golden verification and are unused at runtime (selinux=0).
- **docs:** README with design, roadmap (slab CoW clone, REST service, bootable emit),
  sister-project links; MIT LICENSE.
