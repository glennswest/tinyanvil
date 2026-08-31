# tinyanvil

Hammer out customized [tinystorm](https://github.com/glennswest/tinystorm) goldens:
take the 97 MB base image, add whatever packages a user wants, and seal the
result as a new golden — without ever putting a package manager inside the
image.

```bash
# once: turn a tinystorm build into a base layer
./anvil.sh base /build/images/tinystorm/tinycloudinit-0.7.2.raw tinycloudinit-0.7.2

# per request: base + packages -> new golden (+ license manifest + checksum)
./anvil.sh build tinycloudinit-0.7.2 web-node nginx htop
```

## How it works

A golden is **layers**, laid down by
[stormblock](https://github.com/glennswest/stormblock)'s `golden` writer the
same way container image layers go on — no mkfs, no loop device for the write
path, the ext4 is written directly:

1. **base** — the tinystorm rootfs, captured once as a tar and sealed as a
   read-only golden.
2. **delta** — per request, the base golden is mounted read-only with an
   overlayfs upper on top; `dnf5 --installroot` runs **on the host** against
   the merged view (the same container-style, outside-the-image management
   tinystorm itself is built with). The upper directory *is* the delta.
3. **seal** — `stormblock golden --tar base.tar --tar delta.tar` writes the
   new golden. Every golden ships with a package manifest
   (name / version / arch / license) and a SHA256SUMS entry, preserving
   tinystorm's Fedora license-compliance guarantees; the rpmdb inside stays
   readable from ro mounts.

The write path never touches the base: customization is copy-on-write by
construction, and bases are shared across every golden forged from them.

## Roadmap

- **stormblock slab CoW**: replace the overlayfs step with a true slab CoW
  clone (`stormblock` grows a `clone` verb — goldens imported into a slab,
  cloned, mounted via ublk `attach`). Filed on stormblock.
- **Service**: a small REST API (`POST /api/v1/forge {base, name, packages}`,
  job status, golden registry) in Rust, mkube/microdns-style.
- **Bootable output**: optionally emit a ready-to-boot qcow2 by pairing the
  forged rootfs with tinystorm's ESP/bootloader recipe.
- Overlayfs whiteout handling for package *removals* (deletions in the delta
  layer) — additions-only today.

## Sister projects

- **[tinystorm](https://github.com/glennswest/tinystorm)** — the base: tiniest
  practical Fedora-based cloud image (97 MB qcow2, reproducible from build.sh).
- **[stormblock](https://github.com/glennswest/stormblock)** — pure Rust block
  storage engine; its golden writer and (coming) slab CoW clones are
  tinyanvil's storage layer.
- **[tinycloudinit](https://github.com/glennswest/tinycloudinit)** — the 682 KB
  provisioner inside the base images.
- **[tztiny](https://github.com/glennswest/tztiny)** — 463 KB embedded IANA
  timezone database for images that ship without tzdata.

## License

Scripts are MIT (see `LICENSE`). Forged goldens contain unmodified Fedora
Linux packages under their own licenses — license texts ship inside every
image at `/usr/share/licenses/`, and each golden's manifest lists per-package
licenses. See tinystorm's README for the full compliance statement.
