//! 藏起来的排成树以后（施工 V-46），一级级放行交回的和原来的写法逐项一样：原来的 `walk` 照抄在这里当准绳，照种子造目录树
//! 和藏起来的几条，两边交回的路径（先后也算）、报的错一字不差。
//!
//! 目录树里有：名字是别的名字开头的（`a`、`ab`、`a b`、`a.b`）、只差大小写的、点开头和带 `..` 的名字、不是 UTF-8 的名字、
//! 文件、空目录、指到树里、树外、根目录、不在的地方的链接，偶尔一个列不出来的目录。藏起来的有：树里的目录、文件、链接
//! 本身、经链接走到的、写成尾斜杠、`//`、中间带 `.`、`..` 的，名字只是另一个名字开头的、不在的、整棵树本身、根目录、树外的、
//! 相对路径、重复的。从树的根和树里随便一个目录走起，也照 `plan` 从根目录走起比几回。
//!
//! 平时跑 [`QUICK`] 棵树（调试版不到一秒）；`MIYU_V46_SEEDS=<棵数>` 改，施工时跑过的见施工单 `V-46`。只在 Linux 上编。

use std::ffi::OsString;
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::os::unix::fs::PermissionsExt;

use super::*;

/// 平时跑几棵树。
const QUICK: u64 = 300;

/// 每棵树造几回藏起来的。
const ROUNDS: usize = 12;

/// 原来的写法（施工 V-46 以前的 `walk`），一字不改。
fn walk_old(dir: &Path, hidden: &[PathBuf], out: &mut Vec<PathBuf>) -> Result<(), String> {
    let unreadable = |error: io::Error| format!("cannot open {}: {error}", dir.display());
    for entry in fs::read_dir(dir).map_err(unreadable)? {
        let entry = entry.map_err(unreadable)?;
        let path = entry.path();
        if hidden.contains(&path) {
            continue;
        }
        let kind = entry.file_type().map_err(unreadable)?;
        if kind.is_symlink() {
            continue;
        }
        if kind.is_dir() && hidden.iter().any(|secret| secret.starts_with(&path)) {
            walk_old(&path, hidden, out)?;
        } else {
            out.push(path);
        }
    }
    Ok(())
}

/// 原来的 `plan`：同样换真实的位置、找别名，照原来的写法走。
fn plan_old(hidden: &[PathBuf]) -> Result<Vec<PathBuf>, String> {
    let real = crate::linux::aliases::with_aliases(&reals(hidden)?);
    let root = Path::new("/");
    if real.is_empty() {
        return Ok(vec![root.to_path_buf()]);
    }
    let mut out = Vec::new();
    walk_old(root, &real, &mut out)?;
    Ok(out)
}

/// 照种子走的随机数（xorshift）。
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Rng {
        Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
    }

    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, n: usize) -> usize {
        usize::try_from(self.next() % n as u64).unwrap_or(0)
    }

    fn chance(&mut self, percent: u64) -> bool {
        self.next() % 100 < percent
    }

    fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.below(items.len())]
    }
}

/// 名字从这里挑：互为开头的、只差大小写的、点开头和带点的、不是 UTF-8 的。
const NAMES: &[&[u8]] = &[
    b"a",
    b"ab",
    b"abc",
    b"a b",
    b"a.b",
    b"A",
    b"b",
    b"bc",
    b".x",
    b"..x",
    b"x..",
    b"...",
    "é".as_bytes(),
    b"e\xcc\x81",
    b"\xff",
    b"a\xfe",
    b"a\\b",
];

/// 一棵造出来的树：根、里面的目录（含根）、全部的路径；列不出来的目录删之前改回来。
struct Forest {
    root: PathBuf,
    dirs: Vec<PathBuf>,
    all: Vec<PathBuf>,
    locked: Vec<PathBuf>,
}

impl Forest {
    fn new(seed: u64, rng: &mut Rng) -> Forest {
        let dir = std::env::temp_dir().join(format!(
            "miyu-sandbox-reads-same-{}-{seed}",
            std::process::id()
        ));
        drop(fs::remove_dir_all(&dir));
        fs::create_dir_all(&dir).expect("建得了目录");
        let root = fs::canonicalize(&dir).expect("在");
        let mut forest = Forest {
            root: root.clone(),
            dirs: vec![root.clone()],
            all: Vec::new(),
            locked: Vec::new(),
        };
        forest.grow(&root, 0, rng);
        forest.links(rng);
        if rng.chance(15) && forest.dirs.len() > 1 {
            let dir = forest.dirs[1 + rng.below(forest.dirs.len() - 1)].clone();
            fs::set_permissions(&dir, fs::Permissions::from_mode(0o000)).expect("改得了权限");
            forest.locked.push(dir);
        }
        forest
    }

    fn grow(&mut self, dir: &Path, depth: usize, rng: &mut Rng) {
        let count = rng.below(if depth == 0 { 7 } else { 5 });
        for _ in 0..count {
            let path = dir.join(OsStr::from_bytes(rng.pick(NAMES)));
            if self.all.contains(&path) {
                continue;
            }
            if depth < 4 && rng.chance(50) {
                fs::create_dir(&path).expect("建得了目录");
                self.all.push(path.clone());
                self.dirs.push(path.clone());
                self.grow(&path, depth + 1, rng);
            } else if rng.chance(85) {
                fs::write(&path, b"x").expect("写得进");
                self.all.push(path);
            }
        }
    }

    /// 在随便几个目录里放链接：指到树里的、树本身、根目录、不在的、相对的 `..`。
    fn links(&mut self, rng: &mut Rng) {
        for _ in 0..rng.below(4) {
            let dir = rng.pick(&self.dirs).clone();
            let path = dir.join(OsStr::from_bytes(rng.pick(NAMES)));
            if self.all.contains(&path) {
                continue;
            }
            let target = match rng.below(5) {
                0 if !self.all.is_empty() => rng.pick(&self.all).clone(),
                1 => self.root.clone(),
                2 => PathBuf::from("/"),
                3 => PathBuf::from("no/such"),
                _ => PathBuf::from(".."),
            };
            std::os::unix::fs::symlink(target, &path).expect("建得了链接");
            self.all.push(path);
        }
    }

    /// 一回藏起来的：照上面说的各种写法。
    fn hidden(&self, rng: &mut Rng) -> Vec<PathBuf> {
        let mut out: Vec<PathBuf> = Vec::new();
        for _ in 0..rng.below(7) {
            let some = |rng: &mut Rng| {
                if self.all.is_empty() {
                    self.root.clone()
                } else {
                    rng.pick(&self.all).clone()
                }
            };
            let path = match rng.below(14) {
                0..=3 => some(rng),
                4 => bytes(some(rng), b"/"),
                5 => respell(&some(rng), b"//"),
                6 => respell(&some(rng), b"/./"),
                7 => {
                    let path = some(rng);
                    let name = path.file_name().unwrap_or(OsStr::new("a")).to_owned();
                    path.join("..").join(name)
                }
                8 => {
                    let path = some(rng);
                    let mut name = path
                        .file_name()
                        .map_or(Vec::new(), |name| name.as_bytes().to_vec());
                    name.truncate(name.len().saturating_sub(1).max(1));
                    path.with_file_name(OsString::from_vec(name))
                }
                9 => rng
                    .pick(&self.dirs)
                    .join(OsStr::from_bytes(rng.pick(NAMES))),
                10 => match rng.below(4) {
                    0 => self.root.clone(),
                    1 => PathBuf::from("/"),
                    2 => PathBuf::from("/no/such/place"),
                    _ => PathBuf::from("/usr"),
                },
                11 => some(rng)
                    .strip_prefix(&self.root)
                    .map_or_else(|_| PathBuf::from("a"), Path::to_path_buf),
                12 if !out.is_empty() => rng.pick(&out).clone(),
                _ => some(rng).join(OsStr::from_bytes(rng.pick(NAMES))),
            };
            out.push(path);
        }
        out
    }
}

impl Drop for Forest {
    fn drop(&mut self) {
        for dir in &self.locked {
            drop(fs::set_permissions(dir, fs::Permissions::from_mode(0o755)));
        }
        drop(fs::remove_dir_all(&self.root));
    }
}

/// `path` 后面接上几个字节（尾斜杠）。
fn bytes(path: PathBuf, tail: &[u8]) -> PathBuf {
    let mut raw = path.into_os_string().into_vec();
    raw.extend_from_slice(tail);
    PathBuf::from(OsString::from_vec(raw))
}

/// `path` 里的每个 `/` 换成 `sep`。
fn respell(path: &Path, sep: &[u8]) -> PathBuf {
    let mut raw = Vec::new();
    for &byte in path.as_os_str().as_bytes() {
        if byte == b'/' {
            raw.extend_from_slice(sep);
        } else {
            raw.push(byte);
        }
    }
    PathBuf::from(OsString::from_vec(raw))
}

/// 要跑几棵树：平时 [`QUICK`]，设了 `MIYU_V46_SEEDS` 的照它。
fn seeds() -> u64 {
    std::env::var("MIYU_V46_SEEDS")
        .ok()
        .and_then(|text| text.parse().ok())
        .unwrap_or(QUICK)
}

#[test]
fn the_tree_walks_out_the_same_reads_as_comparing_every_path() {
    let (mut walked, mut failed, mut skipped) = (0_usize, 0_usize, 0_usize);
    let mut denied = false;
    for seed in 0..seeds() {
        let mut rng = Rng::new(seed);
        let forest = Forest::new(seed, &mut rng);
        denied |= forest.locked.iter().any(|dir| fs::read_dir(dir).is_err());
        for round in 0..ROUNDS {
            let hidden = forest.hidden(&mut rng);
            let start = match rng.below(4) {
                0 => rng.pick(&forest.dirs).clone(),
                1 => bytes(forest.root.clone(), b"/"),
                _ => forest.root.clone(),
            };
            let mut old = Vec::new();
            let old = walk_old(&start, &hidden, &mut old).map(|()| old);
            let new = below(&start, &hidden);
            assert_eq!(
                new, old,
                "种子 {seed} 第 {round} 回：从 {start:?} 走，藏起来的 {hidden:?}"
            );
            match &old {
                Ok(reads) => {
                    walked += reads.len();
                    skipped += forest.all.len().saturating_sub(reads.len());
                }
                Err(_) => failed += 1,
            }
            // 照 `plan` 从根目录走（换真实的位置、找别名）：慢一些，少比几回。根目录、临时目录别的程序、别的测试同时在
            // 改，两遍之间列出来的会差，对不上的再比几遍，有一遍一样就算。
            if round == 0 && seed % 5 == 0 {
                let same = (0..5).any(|_| plan(&hidden) == plan_old(&hidden));
                assert!(same, "种子 {seed}：从根目录走，藏起来的 {hidden:?}");
            }
        }
    }
    assert!(walked > 0 && skipped > 0, "放了的、跳过的都有");
    // root 不管权限，改成 000 的目录照样列得出，报错的那一路就比不到。
    if denied {
        assert!(failed > 0, "列不出来的目录两边报一样的错");
    }
}
