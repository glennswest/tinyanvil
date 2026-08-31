#!/usr/bin/env bash
# tinyanvil — hammer out customized tinystorm goldens.
#
# A golden is layers: the tinystorm base rootfs as one tar, the user's
# packages as a delta tar produced through an overlayfs upper, laid down by
# `stormblock golden` exactly the way container layers go on. dnf runs on the
# HOST against the mounted clone (--installroot), never inside the image —
# the same container-style management tinystorm itself is built with.
#
#   anvil.sh base  <tinystorm.raw> <basename>          extract+seal a base
#   anvil.sh build <basename> <newname> [--size 2G] pkg [pkg...]
#   anvil.sh list                                      show bases and goldens
#
# Runs as root on the build box. Store: /build/tinyanvil.
set -euo pipefail

STORE=${TINYANVIL_STORE:-/build/tinyanvil}
SB=${STORMBLOCK:-/root/stormblock/target/x86_64-unknown-linux-musl/release/stormblock}
RELEASEVER=${RELEASEVER:-43}

[ "$(id -u)" = 0 ] || { echo "run as root" >&2; exit 1; }
command -v "$SB" >/dev/null || { echo "stormblock not found at $SB" >&2; exit 1; }
mkdir -p "$STORE"/{bases,goldens,work}

cleanup_dirs=()
cleanup_loops=()
cleanup() {
  set +e
  local d
  for d in "${cleanup_dirs[@]}"; do mountpoint -q "$d" && umount "$d"; done
  for d in "${cleanup_loops[@]}"; do losetup -d "$d" 2>/dev/null; done
}
trap cleanup EXIT

tar_rootfs() { # <dir> <out.tar>
  # security.capability is the one xattr that matters (newuidmap/newgidmap);
  # selinux labels and acls are dead weight in a selinux=0 image and trip
  # stormblock's golden verifier (9 mismatches with them, clean without)
  tar --numeric-owner --xattrs --xattrs-include=security.capability \
      -C "$1" -cf "$2" .
}

case "${1:-}" in
base)
  RAW="$2"; NAME="$3"
  [ -f "$RAW" ] || { echo "no such image: $RAW" >&2; exit 1; }
  MNT="$STORE/work/base-$$"; mkdir -p "$MNT"
  LOOP="$(losetup -Pfr --show "$RAW")"; cleanup_loops+=("$LOOP")
  udevadm settle 2>/dev/null || true
  for _ in $(seq 25); do [ -b "${LOOP}p2" ] && break; sleep 0.2; done
  mount -o ro "${LOOP}p2" "$MNT"; cleanup_dirs+=("$MNT")
  tar_rootfs "$MNT" "$STORE/bases/$NAME.tar"
  umount "$MNT"; losetup -d "$LOOP"; cleanup_loops=(); cleanup_dirs=()
  rmdir "$MNT"
  # seal the base itself as a golden too — it is the ro lower layer for builds
  SIZE_MB=$(( ($(stat -c %s "$STORE/bases/$NAME.tar") / 1048576) * 13 / 10 + 64 ))
  "$SB" golden --out "$STORE/bases/$NAME.img" --size "${SIZE_MB}M" \
        --label root --tar "$STORE/bases/$NAME.tar"
  echo "base '$NAME': $(du -h "$STORE/bases/$NAME.tar" | cut -f1) tar, $(du -h "$STORE/bases/$NAME.img" | cut -f1) golden"
  ;;

build)
  shift; BASE="$1"; NEW="$2"; shift 2
  SIZE=""
  [ "${1:-}" = "--size" ] && { SIZE="$2"; shift 2; }
  [ $# -ge 1 ] || { echo "no packages given" >&2; exit 1; }
  [ -f "$STORE/bases/$BASE.img" ] || { echo "unknown base '$BASE' (run: anvil.sh base ...)" >&2; exit 1; }

  W="$STORE/work/build-$$"
  mkdir -p "$W"/{lower,upper,ovlwork,merged}
  LOOP="$(losetup -fr --show "$STORE/bases/$BASE.img")"; cleanup_loops+=("$LOOP")
  mount -o ro "$LOOP" "$W/lower"; cleanup_dirs+=("$W/lower")
  mount -t overlay overlay \
        -o "lowerdir=$W/lower,upperdir=$W/upper,workdir=$W/ovlwork" \
        "$W/merged"; cleanup_dirs+=("$W/merged")

  dnf5 -y --use-host-config --installroot="$W/merged" --releasever="$RELEASEVER" \
    --setopt=install_weak_deps=0 --setopt=tsflags=nodocs \
    --exclude='linux-firmware*' install "$@"
  dnf5 -y --use-host-config --installroot="$W/merged" --releasever="$RELEASEVER" clean all
  rm -rf "$W/merged"/var/cache/* "$W/merged"/var/lib/dnf
  # keep the image's compliance guarantees: self-contained ro-readable rpmdb + manifest
  sqlite3 "$W/merged/usr/lib/sysimage/rpm/rpmdb.sqlite" 'PRAGMA journal_mode=DELETE; VACUUM;'
  {
    echo "# $NEW — forged from base '$BASE' + $* ($(date -u +%Y-%m-%dT%H:%M:%SZ))"
    echo "# name	version-release	arch	license"
    rpm --root="$W/merged" -qa --qf '%{NAME}\t%{VERSION}-%{RELEASE}\t%{ARCH}\t%{LICENSE}\n' | sort
  } > "$STORE/goldens/$NEW.manifest.txt"

  umount "$W/merged" "$W/lower"; losetup -d "$LOOP"
  cleanup_dirs=(); cleanup_loops=()

  tar_rootfs "$W/upper" "$W/delta.tar"
  if [ -z "$SIZE" ]; then
    BYTES=$(( $(stat -c %s "$STORE/bases/$BASE.tar") + $(stat -c %s "$W/delta.tar") ))
    SIZE="$(( (BYTES / 1048576) * 13 / 10 + 64 ))M"
  fi
  "$SB" golden --out "$STORE/goldens/$NEW.img" --size "$SIZE" --label root \
        --tar "$STORE/bases/$BASE.tar" --tar "$W/delta.tar"
  ( cd "$STORE/goldens" && sha256sum "$NEW.img" "$NEW.manifest.txt" >> SHA256SUMS )
  rm -rf "$W"
  echo "golden '$NEW': $(du -h "$STORE/goldens/$NEW.img" | cut -f1) ($(grep -c $'\t' "$STORE/goldens/$NEW.manifest.txt") packages)"
  ;;

list)
  echo "== bases:";   ls -lh "$STORE/bases"   2>/dev/null | grep -v ^total || true
  echo "== goldens:"; ls -lh "$STORE/goldens" 2>/dev/null | grep -v ^total || true
  ;;

*)
  sed -n '2,14p' "$0"; exit 2 ;;
esac
