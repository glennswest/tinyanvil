# Changelog

## [Unreleased]

### 2026-08-31
- **feat:** Initial tinyforge — forge.sh with `base` (tinystorm raw -> base tar + sealed
  golden via stormblock), `build` (overlayfs + host-side dnf5 -> delta layer -> layered
  golden with license manifest and SHA256SUMS entry), `list`.
- **docs:** README with design, roadmap (slab CoW clone, REST service, bootable emit),
  sister-project links; MIT LICENSE.
