//! 随机对照（施工 V-47）：照种子造一棵目录树和一份规格，再照种子一步步改（夹着各种过期的情形），每一步缓存交出来的那一份
//! 建的时候加的规则，和此刻从头排、从头打开查的逐条一样（[`check`]）。从头建当准绳，缓存少比了哪一样，总有一步对不上。
//!
//! 改法：藏起来的上级、更上级里新建（文件、目录、链接）、删、改名、同名换成新的（文件换目录、目录换文件、目录换新目录）；放读的
//! 挪进藏起来的里面、原处另建同名的；藏起来的和旁边的互换；路上的目录换成链接、链接改指；能写的删了重建、换成文件；放开只读的
//! 删了重建；把修改时间拨回去；路上的目录列不出来、又列得出来；规格变（多藏一条、少藏一条、换能写的、加放开只读的、只换
//! 套接字）；什么都不改（要从缓存交）。
//!
//! 平时 [`QUICK`] 棵树、每棵 [`STEPS`] 步（调试版几秒）；`MIYU_V47_SEEDS=<棵数>` 改，施工时跑过的见施工单 `V-47`。只在 Linux
//! 上编：Windows、macOS 的 CI 不跑它。

use super::*;

/// 平时跑几棵树。
const QUICK: u64 = 40;

/// 每棵树改几步。
const STEPS: usize = 16;

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
        usize::try_from(self.next() % u64::try_from(n.max(1)).unwrap_or(1)).unwrap_or(0)
    }

    fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.below(items.len())]
    }
}

/// 名字从这里挑：互为开头的、点开头的、带空格的。
const NAMES: &[&str] = &["a", "ab", "a b", ".x", "b", "bc", "c", "d"];

/// 一棵树和照它的规格。
struct Case {
    tree: Tree,
    spec: Spec,
    rng: Rng,
}

impl Case {
    fn new(seed: u64) -> Case {
        let tree = Tree::new("same");
        let mut rng = Rng::new(seed);
        // 三层目录、每层几样。
        for _ in 0..12 {
            let path = random_path(&mut rng, 3);
            let at = tree.0.join(&path);
            if let Some(parent) = at.parent() {
                drop(std::fs::create_dir_all(parent));
            }
            if rng.below(3) == 0 {
                drop(std::fs::write(&at, "f"));
            } else {
                drop(std::fs::create_dir_all(&at));
            }
        }
        let mut case = Case {
            tree,
            spec: Spec {
                write: Vec::new(),
                hidden: Vec::new(),
                read: None,
                reveal: Vec::new(),
                sockets: Vec::new(),
            },
            rng,
        };
        for _ in 0..1 + case.rng.below(3) {
            let hidden = case.any();
            case.spec.hidden.push(hidden);
        }
        for _ in 0..case.rng.below(3) {
            let write = case.any();
            case.spec.write.push(write);
        }
        if case.rng.below(3) == 0 {
            let reveal = case.spec.hidden[0].join(*case.rng.pick(NAMES));
            case.spec.reveal.push(reveal);
        }
        case
    }

    /// 树里随便一处（在不在都行）。
    fn any(&mut self) -> PathBuf {
        let path = random_path(&mut self.rng, 3);
        self.tree.0.join(path)
    }

    /// 树里一处在的（不是树本身），找不到的交回空的。
    fn existing(&mut self) -> Option<PathBuf> {
        (0..20).find_map(|_| {
            let path = self.any();
            std::fs::symlink_metadata(&path).is_ok().then_some(path)
        })
    }

    /// 藏起来的上级、更上级（树里的）里随便一处要放的名字。
    fn beside_hidden(&mut self) -> PathBuf {
        let hidden = self.rng.pick(&self.spec.hidden).clone();
        let mut dirs: Vec<PathBuf> = hidden
            .ancestors()
            .skip(1)
            .take_while(|dir| dir.starts_with(&self.tree.0))
            .map(Path::to_path_buf)
            .collect();
        dirs.push(self.tree.0.clone());
        let dir = self.rng.pick(&dirs).clone();
        dir.join(*self.rng.pick(NAMES))
    }

    /// 改一步，交回改的是什么（出错时看）。改不成的（目录不在之类）不要紧：照样比。
    fn step(&mut self) -> String {
        let kind = self.rng.below(20);
        let at = self.beside_hidden();
        let other = self.any();
        let what = format!("{kind} {} {}", at.display(), other.display());
        match kind {
            0 => drop(std::fs::create_dir_all(&at)),
            1 => drop(std::fs::write(&at, "new")),
            2 => drop(std::os::unix::fs::symlink(&other, &at)),
            3 => remove(&at),
            4 => drop(std::fs::rename(&at, &other)),
            5 => {
                // 同名换成新的：文件换目录、目录换文件、目录换新目录。
                let was_dir = at.is_dir();
                remove(&at);
                if was_dir && self.rng.below(2) == 0 {
                    drop(std::fs::write(&at, "replaced"));
                } else {
                    drop(std::fs::create_dir_all(&at));
                }
            }
            6 => {
                // 放读的挪进藏起来的里面，原处另建同名的。
                let hidden = self.rng.pick(&self.spec.hidden).clone();
                drop(std::fs::create_dir_all(&hidden));
                drop(std::fs::rename(&at, hidden.join("moved")));
                drop(std::fs::create_dir_all(&at));
            }
            7 => {
                // 藏起来的和旁边的互换。
                let hidden = self.rng.pick(&self.spec.hidden).clone();
                let parked = self.tree.0.join("parked");
                remove(&parked);
                drop(std::fs::rename(&hidden, &parked));
                drop(std::fs::rename(&at, &hidden));
                drop(std::fs::rename(&parked, &at));
            }
            8 => {
                // 路上的目录换成链接（指向挪走的它，或者别处）。
                let moved = self.tree.0.join("moved-dir");
                remove(&moved);
                if std::fs::rename(&at, &moved).is_ok() {
                    let target = if self.rng.below(2) == 0 { moved } else { other };
                    drop(std::os::unix::fs::symlink(target, &at));
                }
            }
            9 => {
                if let Some(write) = self.spec.write.first().cloned() {
                    remove(&write);
                    if self.rng.below(3) == 0 {
                        drop(std::fs::write(&write, "w"));
                    } else {
                        drop(std::fs::create_dir_all(&write));
                    }
                }
            }
            10 => {
                if let Some(reveal) = self.spec.reveal.first().cloned() {
                    remove(&reveal);
                    drop(std::fs::create_dir_all(&reveal));
                }
            }
            11 => {
                drop(std::fs::write(at.with_extension("t"), "t"));
                let dir = at.parent().unwrap_or(&self.tree.0).to_path_buf();
                drop(
                    Command::new("touch")
                        .args(["-d", "@1000000000"])
                        .arg(dir)
                        .stderr(std::process::Stdio::null())
                        .status(),
                );
            }
            12 => {
                let dir = at.parent().unwrap_or(&self.tree.0).to_path_buf();
                if dir != self.tree.0 {
                    drop(
                        Command::new("chmod")
                            .arg("000")
                            .arg(&dir)
                            .stderr(std::process::Stdio::null())
                            .status(),
                    );
                }
            }
            13 => {
                drop(
                    Command::new("chmod")
                        .arg("-R")
                        .arg("u+rwx")
                        .arg(&self.tree.0)
                        .stderr(std::process::Stdio::null())
                        .status(),
                );
            }
            14 => {
                if self.rng.below(2) == 0 || self.spec.hidden.len() < 2 {
                    self.spec.hidden.push(other);
                } else {
                    self.spec.hidden.pop();
                }
            }
            15 => {
                if self.spec.write.is_empty() {
                    self.spec.write.push(other);
                } else {
                    self.spec.write.clear();
                }
            }
            16 => {
                let reveal = self.rng.pick(&self.spec.hidden).join(*self.rng.pick(NAMES));
                self.spec.reveal = vec![reveal];
            }
            17 => self.spec.sockets = vec![other],
            18 => {
                if let Some(path) = self.existing() {
                    remove(&path);
                }
            }
            _ => {}
        }
        what
    }
}

/// 删掉，是目录的连里面的。
fn remove(path: &Path) {
    match std::fs::symlink_metadata(path) {
        Ok(meta) if meta.is_dir() => drop(std::fs::remove_dir_all(path)),
        Ok(_) => drop(std::fs::remove_file(path)),
        Err(_) => {}
    }
}

/// 最多 `depth` 段的随机相对路径。
fn random_path(rng: &mut Rng, depth: usize) -> PathBuf {
    let mut path = PathBuf::new();
    for _ in 0..1 + rng.below(depth) {
        path.push(*rng.pick(NAMES));
    }
    path
}

#[test]
fn the_cache_serves_what_a_fresh_build_would_add_right_now() {
    let Some(_serial) = usable() else {
        return;
    };
    let seeds = std::env::var("MIYU_V47_SEEDS")
        .ok()
        .and_then(|seeds| seeds.parse().ok())
        .unwrap_or(QUICK);
    let (mut served, mut built) = (0_u64, 0_u64);
    for seed in 0..seeds {
        let mut case = Case::new(seed);
        let cache = Cache::new();
        check(&cache, &case.spec, &format!("种子 {seed} 头一次"));
        for at in 0..STEPS {
            let what = case.step();
            let spec = case.spec.clone();
            match check(
                &cache,
                &spec,
                &format!("种子 {seed} 第 {at} 步：{what}；{spec:?}"),
            ) {
                true => built += 1,
                false => served += 1,
            }
        }
    }
    eprintln!("{seeds} 棵：从缓存交 {served} 次、新建 {built} 次");
    // 有从缓存交的（不然对照的只是新建的）。
    assert!(
        served > 0 && built > 0,
        "从缓存交 {served} 次、新建 {built} 次"
    );
}
