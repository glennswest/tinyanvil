# Changelog

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
