# tinyforge — project context

Forge customized tinystorm goldens: base rootfs + user packages -> new sealed
golden, via stormblock's golden layer writer. dnf runs host-side against an
overlayfs merge (container-style); the delta upper becomes the second layer.
See README.md.

## Version
- Current: 0.1.0 (locations: `VERSION`, `CHANGELOG.md` heading)

## Build / run
- Runs as root on `root@dev.g8.lo`; store is `/build/tinyforge` (bases/, goldens/, work/).
- stormblock binary: `/root/stormblock/target/x86_64-unknown-linux-musl/release/stormblock`.
- Working tree on dev: `/root/tinyforge`.

## Work plan
- [x] v0.1.0: forge.sh — `base` (raw -> base tar + sealed golden), `build`
      (overlayfs + host dnf5 -> delta tar -> layered golden + manifest + sha256), `list`
- [ ] Verify end-to-end on dev (base from tinycloudinit-0.7.2.raw, build with htop)
- [ ] stormblock issue: `clone` verb (import golden into slab, CoW clone, attach) — then
      swap the overlayfs step for real slab CoW
- [ ] REST service (Rust): POST /api/v1/forge, job queue, golden registry
- [ ] Bootable qcow2 emit (pair forged rootfs with tinystorm ESP/bootloader recipe)
- [ ] Whiteout handling for package removals in the delta layer

## Notes / decisions
- Additions-only in v0.1: overlayfs whiteouts (deleted base files) ride the delta tar as
  0:0 char devices; whether stormblock's layer writer interprets them is untested.
- Base tar captured with --numeric-owner --xattrs --acls --selinux to keep capabilities
  (e.g. filecaps on ping) intact.
- Same compliance invariants as tinystorm: manifest per golden, rpmdb VACUUM +
  journal_mode=DELETE so ro mounts can query it.
