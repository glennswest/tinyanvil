// tinyanvil — hammer out customized tinystorm goldens.
//
// A golden is layers: the tinystorm base rootfs as one tar, the user's
// packages as a delta produced through an overlayfs upper, laid down by
// `stormblock golden` the way container layers go on. dnf runs on the HOST
// against the merged view (--installroot), never inside the image.
//
// The binary orchestrates the host tools that must exist anyway (dnf5,
// stormblock, tar, mount, losetup, sqlite3, rpm); mounts and loop devices
// are RAII guards so no failure path leaks them.
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{exit, Command};

type R<T> = Result<T, String>;

fn store() -> PathBuf {
    PathBuf::from(env::var("TINYANVIL_STORE").unwrap_or("/build/tinyanvil".into()))
}
fn stormblock() -> String {
    env::var("STORMBLOCK")
        .unwrap_or("/root/stormblock/target/x86_64-unknown-linux-musl/release/stormblock".into())
}
fn releasever() -> String {
    env::var("RELEASEVER").unwrap_or("43".into())
}

/// Capture stdout; error carries the tool's stderr.
fn sh(desc: &str, cmd: &mut Command) -> R<String> {
    let out = cmd.output().map_err(|e| format!("{desc}: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "{desc}: {}\n{}",
            out.status,
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Stream output to the terminal (dnf transactions, stormblock builds).
fn run(desc: &str, cmd: &mut Command) -> R<()> {
    let st = cmd.status().map_err(|e| format!("{desc}: {e}"))?;
    if !st.success() {
        return Err(format!("{desc}: {st}"));
    }
    Ok(())
}

struct LoopDev(String);
impl LoopDev {
    fn attach(img: &Path, partitioned: bool) -> R<LoopDev> {
        let mut c = Command::new("losetup");
        c.arg(if partitioned { "-Pfr" } else { "-fr" }).arg("--show").arg(img);
        let dev = sh("losetup", &mut c)?.trim().to_string();
        Ok(LoopDev(dev))
    }
    /// Wait for a partition node to appear (udev races with other builds).
    fn part(&self, n: u32) -> R<String> {
        let p = format!("{}p{n}", self.0);
        let _ = Command::new("udevadm").arg("settle").status();
        for _ in 0..50 {
            if Path::new(&p).exists() {
                return Ok(p);
            }
            std::thread::sleep(std::time::Duration::from_millis(200));
        }
        Err(format!("partition {p} never appeared"))
    }
}
impl Drop for LoopDev {
    fn drop(&mut self) {
        let _ = Command::new("losetup").args(["-d", &self.0]).status();
    }
}

struct Mnt(PathBuf);
impl Mnt {
    fn ro(dev: &str, at: &Path) -> R<Mnt> {
        run("mount", Command::new("mount").args(["-o", "ro", dev]).arg(at))?;
        Ok(Mnt(at.to_path_buf()))
    }
    fn overlay(lower: &Path, upper: &Path, work: &Path, at: &Path) -> R<Mnt> {
        let opt = format!(
            "lowerdir={},upperdir={},workdir={}",
            lower.display(),
            upper.display(),
            work.display()
        );
        run(
            "mount overlay",
            Command::new("mount").args(["-t", "overlay", "overlay", "-o", &opt]).arg(at),
        )?;
        Ok(Mnt(at.to_path_buf()))
    }
}
impl Drop for Mnt {
    fn drop(&mut self) {
        let _ = Command::new("umount").arg(&self.0).status();
    }
}

/// security.capability is the one xattr that matters (newuidmap/newgidmap);
/// selinux labels and acls trip stormblock's golden verifier and are dead
/// weight in a selinux=0 image.
fn tar_rootfs(dir: &Path, out: &Path) -> R<()> {
    run(
        "tar",
        Command::new("tar")
            .args(["--numeric-owner", "--xattrs", "--xattrs-include=security.capability", "-C"])
            .arg(dir)
            .arg("-cf")
            .arg(out)
            .arg("."),
    )
}

fn golden(out: &Path, size_mib: u64, tars: &[&Path]) -> R<()> {
    let mut c = Command::new(stormblock());
    c.arg("golden").arg("--out").arg(out).arg("--size").arg(format!("{size_mib}M"));
    c.args(["--label", "root"]);
    for t in tars {
        c.arg("--tar").arg(t);
    }
    run("stormblock golden", &mut c)
}

/// Known repo aliases resolve to their release RPM, which installs the .repo
/// file and GPG keys into the image the proper way. EPEL is EL-only: its
/// packages are *built from* Fedora, so a Fedora image never uses it.
fn repo_release_url(alias: &str, rv: &str) -> R<String> {
    match alias {
        "rpmfusion-free" => Ok(format!(
            "https://mirrors.rpmfusion.org/free/fedora/rpmfusion-free-release-{rv}.noarch.rpm"
        )),
        "rpmfusion-nonfree" => Ok(format!(
            "https://mirrors.rpmfusion.org/nonfree/fedora/rpmfusion-nonfree-release-{rv}.noarch.rpm"
        )),
        e if e.starts_with("epel") => Err(
            "EPEL is for RHEL/CentOS/Alma, not Fedora — Fedora already carries those packages \
             (EPEL builds from Fedora sources). Try rpmfusion-free / rpmfusion-nonfree, or pass \
             a release-RPM URL.".into(),
        ),
        u if u.starts_with("http://") || u.starts_with("https://") => Ok(u.to_string()),
        other => Err(format!(
            "unknown repo '{other}' — known: rpmfusion-free, rpmfusion-nonfree, or a release-RPM URL"
        )),
    }
}

/// dnf5 resolves `gpgkey=file:///...` against the HOST, not the installroot.
/// Rewrite every such key in the image's .repo files to point inside it.
fn gpgkey_overrides(merged: &Path) -> Vec<String> {
    let mut out = Vec::new();
    let Ok(rd) = fs::read_dir(merged.join("etc/yum.repos.d")) else {
        return out;
    };
    for e in rd.flatten() {
        let Ok(text) = fs::read_to_string(e.path()) else {
            continue;
        };
        let mut section = String::new();
        for line in text.lines() {
            let l = line.trim();
            if l.starts_with('[') && l.ends_with(']') {
                section = l[1..l.len() - 1].to_string();
            } else if let Some(v) = l.strip_prefix("gpgkey=") {
                if v.contains("file:///") && !section.is_empty() {
                    let rewritten =
                        v.replace("file:///", &format!("file://{}/", merged.display()));
                    out.push(format!("--setopt={section}.gpgkey={rewritten}"));
                }
            }
        }
    }
    out
}

fn fsize(p: &Path) -> R<u64> {
    Ok(fs::metadata(p).map_err(|e| format!("{}: {e}", p.display()))?.len())
}
fn auto_size_mib(bytes: u64) -> u64 {
    (bytes / (1 << 20)) * 13 / 10 + 64
}
fn human(b: u64) -> String {
    if b >= 1 << 30 {
        format!("{:.1}G", b as f64 / (1 << 30) as f64)
    } else {
        format!("{}M", b >> 20)
    }
}

fn base(raw: &Path, name: &str) -> R<()> {
    let s = store();
    fs::create_dir_all(s.join("bases")).and(fs::create_dir_all(s.join("work"))).map_err(|e| e.to_string())?;
    let tar = s.join("bases").join(format!("{name}.tar"));
    let mnt = s.join("work").join(format!("base-{}", std::process::id()));
    fs::create_dir_all(&mnt).map_err(|e| e.to_string())?;

    let lo = LoopDev::attach(raw, true)?;
    let p2 = lo.part(2)?;
    {
        let _m = Mnt::ro(&p2, &mnt)?;
        tar_rootfs(&mnt, &tar)?;
    } // unmount before detach
    drop(lo);
    let _ = fs::remove_dir(&mnt);

    let img = s.join("bases").join(format!("{name}.img"));
    golden(&img, auto_size_mib(fsize(&tar)?), &[&tar])?;
    println!("base '{name}': {} tar, {} golden", human(fsize(&tar)?), human(fsize(&img)?));
    Ok(())
}

fn build(base: &str, new: &str, size_mib: Option<u64>, repos: &[String], pkgs: &[String]) -> R<()> {
    let s = store();
    let base_tar = s.join("bases").join(format!("{base}.tar"));
    let base_img = s.join("bases").join(format!("{base}.img"));
    if !base_img.exists() {
        return Err(format!("unknown base '{base}' (run: tinyanvil base <raw> <name>)"));
    }
    fs::create_dir_all(s.join("goldens")).map_err(|e| e.to_string())?;
    let w = s.join("work").join(format!("build-{}", std::process::id()));
    for d in ["lower", "upper", "ovlwork", "merged"] {
        fs::create_dir_all(w.join(d)).map_err(|e| e.to_string())?;
    }

    let manifest = s.join("goldens").join(format!("{new}.manifest.txt"));
    let npkgs;
    {
        let lo = LoopDev::attach(&base_img, false)?;
        let _lower = Mnt::ro(&lo.0, &w.join("lower"))?;
        let merged = w.join("merged");
        let _ovl = Mnt::overlay(&w.join("lower"), &w.join("upper"), &w.join("ovlwork"), &merged)?;

        let rv = releasever();
        if !repos.is_empty() {
            // release RPMs drop .repo + GPG keys into the image; bootstrapped
            // with the host's repo config since the extras don't exist yet
            let urls: Vec<String> =
                repos.iter().map(|r| repo_release_url(r, &rv)).collect::<R<_>>()?;
            run(
                "dnf5 install repo releases",
                Command::new("dnf5")
                    .args(["-y", "--use-host-config"])
                    .arg(format!("--releasever={rv}"))
                    .arg(format!("--installroot={}", merged.display()))
                    .args(["--setopt=install_weak_deps=0", "install"])
                    .args(&urls),
            )?;
        }
        // with extra repos, resolve against the image's own repo config so the
        // just-installed .repo files (and the base's fedora repos) are used
        let mut inst = Command::new("dnf5");
        inst.arg("-y");
        if repos.is_empty() {
            inst.arg("--use-host-config");
        } else {
            for o in gpgkey_overrides(&merged) {
                inst.arg(o);
            }
        }
        inst.arg(format!("--releasever={rv}"))
            .arg(format!("--installroot={}", merged.display()))
            .args(["--setopt=install_weak_deps=0", "--setopt=tsflags=nodocs"])
            .args(["--exclude=linux-firmware*", "install"])
            .args(pkgs);
        run("dnf5 install", &mut inst)?;
        run(
            "dnf5 clean",
            Command::new("dnf5")
                .args(["-y", "--use-host-config"])
                .arg(format!("--releasever={rv}"))
                .arg(format!("--installroot={}", merged.display()))
                .args(["clean", "all"]),
        )?;
        for junk in ["var/cache", "var/lib/dnf"] {
            let _ = fs::remove_dir_all(merged.join(junk));
            let _ = fs::create_dir_all(merged.join("var/cache"));
        }
        // keep the compliance invariants: self-contained ro-readable rpmdb + manifest
        run(
            "sqlite3 vacuum",
            Command::new("sqlite3")
                .arg(merged.join("usr/lib/sysimage/rpm/rpmdb.sqlite"))
                .arg("PRAGMA journal_mode=DELETE; VACUUM;"),
        )?;
        let list = sh(
            "rpm manifest",
            Command::new("rpm")
                .arg(format!("--root={}", merged.display()))
                .args(["-qa", "--qf", "%{NAME}\t%{VERSION}-%{RELEASE}\t%{ARCH}\t%{LICENSE}\n"]),
        )?;
        let mut lines: Vec<&str> = list.lines().collect();
        lines.sort_unstable();
        npkgs = lines.len();
        let stamp = sh("date", Command::new("date").args(["-u", "+%Y-%m-%dT%H:%M:%SZ"]))?;
        fs::write(
            &manifest,
            format!(
                "# {new} — forged from base '{base}' + {}{} ({})\n# name\tversion-release\tarch\tlicense\n{}\n",
                pkgs.join(" "),
                if repos.is_empty() { String::new() } else { format!(" [repos: {}]", repos.join(",")) },
                stamp.trim(),
                lines.join("\n")
            ),
        )
        .map_err(|e| e.to_string())?;
    } // overlay + lower unmount, loop detaches

    // the base earned its size the hard way — don't let the delta undo it:
    // strip the same locale/doc weight from what the new packages brought.
    // (pure additions live in the upper dir, so deleting there is safe and
    // can never whiteout base content)
    let upper = w.join("upper");
    for junk in ["usr/share/locale", "usr/share/doc", "usr/share/man", "usr/share/info"] {
        let _ = fs::remove_dir_all(upper.join(junk));
    }
    let delta = w.join("delta.tar");
    tar_rootfs(&upper, &delta)?;
    let size = match size_mib {
        Some(m) => m,
        None => auto_size_mib(fsize(&base_tar)? + fsize(&delta)?),
    };
    let img = s.join("goldens").join(format!("{new}.img"));
    golden(&img, size, &[&base_tar, &delta])?;
    run(
        "sha256sum",
        Command::new("sh")
            .arg("-c")
            .arg(format!("sha256sum '{new}.img' '{new}.manifest.txt' >> SHA256SUMS"))
            .current_dir(s.join("goldens")),
    )?;
    let _ = fs::remove_dir_all(&w);
    println!(
        "golden '{new}': {} ({npkgs} packages, {} delta)",
        human(fsize(&img)?),
        human(fsize(&delta).unwrap_or(0))
    );
    Ok(())
}

fn list() -> R<()> {
    for (title, dir) in [("bases", "bases"), ("goldens", "goldens")] {
        println!("== {title}:");
        if let Ok(rd) = fs::read_dir(store().join(dir)) {
            let mut rows: Vec<(String, u64)> = rd
                .flatten()
                .filter_map(|e| Some((e.file_name().into_string().ok()?, e.metadata().ok()?.len())))
                .collect();
            rows.sort();
            for (n, sz) in rows {
                println!("  {:>8}  {n}", human(sz));
            }
        }
    }
    Ok(())
}

fn parse_size(s: &str) -> Option<u64> {
    let (num, mul) = match s.as_bytes().last()? {
        b'G' | b'g' => (&s[..s.len() - 1], 1024),
        b'M' | b'm' => (&s[..s.len() - 1], 1),
        _ => (s, 1),
    };
    num.parse::<u64>().ok().map(|n| n * mul)
}

fn main() {
    if unsafe { libc_geteuid() } != 0 {
        eprintln!("run as root");
        exit(1);
    }
    let args: Vec<String> = env::args().skip(1).collect();
    let r = match args.first().map(String::as_str) {
        Some("base") if args.len() == 3 => base(Path::new(&args[1]), &args[2]),
        Some("build") if args.len() >= 4 => {
            let (base_name, new) = (&args[1], &args[2]);
            let mut rest = &args[3..];
            let mut size = None;
            let mut repos: Vec<String> = Vec::new();
            loop {
                match rest.first().map(String::as_str) {
                    Some("--size") if rest.len() >= 2 => {
                        size = parse_size(&rest[1]);
                        if size.is_none() {
                            eprintln!("bad --size '{}'", rest[1]);
                            exit(2);
                        }
                        rest = &rest[2..];
                    }
                    Some("--repo") if rest.len() >= 2 => {
                        repos.push(rest[1].clone());
                        rest = &rest[2..];
                    }
                    _ => break,
                }
            }
            if rest.is_empty() {
                eprintln!("no packages given");
                exit(2);
            }
            build(base_name, new, size, &repos, rest)
        }
        Some("list") => list(),
        Some("--version") | Some("-V") => {
            println!("tinyanvil {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        _ => {
            eprintln!(
                "usage: tinyanvil base <tinystorm.raw> <basename>\n\
                 \x20      tinyanvil build <basename> <newname> [--size 2G] [--repo rpmfusion-free]... <pkg>...\n\
                 \x20      tinyanvil list"
            );
            exit(2);
        }
    };
    if let Err(e) = r {
        eprintln!("tinyanvil: {e}");
        exit(1);
    }
}

extern "C" {
    #[link_name = "geteuid"]
    fn libc_geteuid() -> u32;
}
