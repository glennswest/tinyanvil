# tinyanvil — project context

Forge customized tinystorm goldens: base rootfs + user packages -> new sealed
golden, via stormblock's golden layer writer. dnf runs host-side against an
overlayfs merge (container-style); the delta upper becomes the second layer.
See README.md.

## Version
- Current: 0.4.0 (locations: `Cargo.toml`, `VERSION`, `CHANGELOG.md` heading)

## Build / run
- Rust binary (std-only): `CARGO_TARGET_DIR=/build/cargo/tinyanvil cargo build --release
  --target x86_64-unknown-linux-musl` on dev. Never build on the Mac.
- Runs as root on `root@dev.g8.lo`; store is `/build/tinyanvil` (bases/, goldens/, work/).
- stormblock binary: `/root/stormblock/target/x86_64-unknown-linux-musl/release/stormblock`.
- Working tree on dev: `/root/tinyanvil`.

## Work plan
- [x] v0.3.0: Rust rewrite of the v0.1 shell prototype (std-only, RAII mount/loop guards)
- [x] v0.1.0: anvil.sh — `base` (raw -> base tar + sealed golden), `build`
      (overlayfs + host dnf5 -> delta tar -> layered golden + manifest + sha256), `list`
- [x] Verified end-to-end on dev (2026-08-31): base from tinycloudinit-0.7.2.raw
      (123M tar / 222M golden, 5252 entries, verifier clean), `build web-node nginx htop`
      -> 248M golden, 124 packages, nginx+htop confirmed present via ro mount + rpm query
- [ ] stormblock issue: `clone` verb (import golden into slab, CoW clone, attach) — then
      swap the overlayfs step for real slab CoW
- [x] v0.4.0: --repo (release-RPM mechanism, rpmfusion aliases, EPEL refused with
      explanation) + delta-layer locale/doc/man/info strip + delta size reporting
- [x] v0.4.1: keep <name>.delta.tar + <name>.meta (base/repos/packages/built/delta size)
      beside each golden, all in SHA256SUMS. Decision: flat/promoted is the default form;
      --nested + `promote` wait for stormblock slab CoW (#87).
- [ ] REST service `tinyanvil serve` (Rust): POST /api/v1/build, job queue, golden
      registry — runs on StormCOS; GUI ships as a stormconsole plugin driving that API
- [ ] Bootable qcow2 emit (pair forged rootfs with tinystorm ESP/bootloader recipe)
- [ ] Whiteout handling for package removals in the delta layer

## Notes / decisions
- Additions-only in v0.1: overlayfs whiteouts (deleted base files) ride the delta tar as
  0:0 char devices; whether stormblock's layer writer interprets them is untested.
- Tar captures --numeric-owner + security.capability xattr ONLY (newuidmap/newgidmap caps).
  selinux labels/acls trip stormblock's golden verifier (9 mismatches) and are dead weight
  in a selinux=0 image. Verifier does not name failing entries — issue filed on stormblock.
- Same compliance invariants as tinystorm: manifest per golden, rpmdb VACUUM +
  journal_mode=DELETE so ro mounts can query it.
