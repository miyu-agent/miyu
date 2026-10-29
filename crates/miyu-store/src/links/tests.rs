//! 列一层目录、认链接（施工 X-37）：链接照指到的认种类，位置是链接本身、另记真实的位置；断了的、绕回自己的跳过说为什么；
//! 真实位置在数据根里的只认自己放东西的几处，所在的目录是链接的照样查；不碰数据根的哪里都认。Windows 上造不了链接的
//! 跳过（CI 上造得了）。

use std::fs;

use super::*;
use crate::env::{Env, Platform};
use crate::test_support::{Scratch, link};

struct Site {
    scratch: Scratch,
    root: DataRoot,
}

impl Site {
    /// 数据根在 `data/`。
    fn new() -> Site {
        let scratch = Scratch::new();
        let env = Env {
            platform: Platform::current(),
            miyu_home: Some(scratch.path().join("data").into_os_string()),
            home: None,
            xdg_cache_home: None,
            local_app_data: None,
            miyu_resources: None,
            exe: None,
        };
        Site {
            root: DataRoot::locate(&env).unwrap(),
            scratch,
        }
    }

    fn at(&self, path: &str) -> PathBuf {
        self.scratch.path().join(path)
    }

    /// 造一个文件，上级目录跟着建。
    fn file(&self, path: &str) -> PathBuf {
        let path = self.at(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "x").unwrap();
        path
    }

    fn dir(&self, path: &str) -> PathBuf {
        let path = self.at(path);
        fs::create_dir_all(&path).unwrap();
        path
    }

    fn fence(&self) -> Fence {
        Fence::of(&self.root)
    }
}

fn names(entries: &[Entry]) -> Vec<&str> {
    entries.iter().map(|entry| entry.name.as_str()).collect()
}

fn real(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap()
}

#[test]
fn a_link_counts_as_what_it_points_to() {
    let site = Site::new();
    let place = site.dir("data/scripts");
    site.file("data/scripts/plain.py");
    let file = site.file("elsewhere/tool.py");
    let dir = site.dir("elsewhere/notes");
    if !(link(&file, &place.join("tool.py")) && link(&dir, &place.join("notes"))) {
        return;
    }
    let (files, skipped) = list(&place, Kind::File, &site.fence(), |_| true);
    assert_eq!(names(&files), ["plain.py", "tool.py"], "指到目录的不算文件");
    assert_eq!(files[0].real, None, "不是链接的没有真实位置");
    assert_eq!(
        (files[1].path.clone(), files[1].real.clone()),
        (place.join("tool.py"), Some(real(&file))),
        "位置是链接本身，另记指到的"
    );
    assert!(skipped.is_empty(), "{skipped:?}");
    let (dirs, _) = list(&place, Kind::Dir, &site.fence(), |_| true);
    assert_eq!(names(&dirs), ["notes"]);
    assert_eq!(dirs[0].real, Some(real(&dir)));
}

#[test]
fn a_broken_or_looping_link_is_skipped_and_said_why() {
    let site = Site::new();
    let place = site.dir("data/skills");
    let (broken, looping) = (place.join("gone"), place.join("self"));
    if !(link(&site.at("nowhere"), &broken) && link(&looping, &looping)) {
        return;
    }
    let (found, skipped) = list(&place, Kind::Dir, &site.fence(), |_| true);
    assert!(found.is_empty(), "{found:?}");
    let said: Vec<(&Path, bool, bool)> = skipped
        .iter()
        .map(|one| {
            (
                one.path.as_path(),
                one.why.starts_with("broken link: "),
                one.outside.is_none(),
            )
        })
        .collect();
    assert_eq!(
        said,
        [
            (broken.as_path(), true, true),
            (looping.as_path(), true, true)
        ]
    );
}

#[test]
fn inside_the_data_root_only_this_accounts_own_folders_count() {
    let site = Site::new();
    let place = site.dir("data/scripts");
    let own = site.file("data/packages/kit/scripts/run.py");
    let system = site.file("data/packages/kit/scripts/sys.py");
    let secret = site.file("data/secrets.toml");
    let theirs = site.file("data/home/bob/scripts/theirs.py");
    let free = site.file("elsewhere/free.py");
    for (target, name) in [
        (&own, "own.py"),
        (&system, "sys.py"),
        (&secret, "secret.py"),
        (&theirs, "theirs.py"),
        (&free, "free.py"),
    ] {
        if !link(target, &place.join(name)) {
            return;
        }
    }
    let (found, skipped) = list(&place, Kind::File, &site.fence(), |_| true);
    assert_eq!(
        names(&found),
        ["free.py", "own.py", "sys.py"],
        "数据根外面的、自己的包里的、系统那一层的包里的认（施工 U23 上上）"
    );
    let outside: Vec<(PathBuf, Option<PathBuf>)> = skipped
        .iter()
        .map(|one| (one.path.clone(), one.outside.clone()))
        .collect();
    assert_eq!(
        outside,
        [
            (place.join("secret.py"), Some(real(&secret))),
            (place.join("theirs.py"), Some(real(&theirs))),
        ],
        "system/、别的账号的不认，带真实位置"
    );
    let (found, skipped) = list(&place, Kind::File, &Fence::anywhere(), |_| true);
    assert_eq!(found.len(), 5, "不碰数据根的哪里都认：{found:?}");
    assert!(skipped.is_empty(), "{skipped:?}");
}

#[test]
fn a_folder_that_is_itself_a_link_is_checked_where_it_really_is() {
    let site = Site::new();
    site.dir("data/home/admin");
    let token = site.file("data/keys/token");
    let place = site.at("data/scripts");
    if !link(&site.at("data/keys"), &place) {
        return;
    }
    let (found, skipped) = list(&place, Kind::File, &site.fence(), |_| true);
    assert!(found.is_empty(), "{found:?}");
    assert_eq!(skipped.len(), 1, "{skipped:?}");
    assert_eq!(skipped[0].outside, Some(real(&token)));
}

#[test]
fn only_wanted_names_are_looked_at() {
    let site = Site::new();
    let place = site.dir("data/commands");
    site.file("data/commands/notes.md");
    site.file("data/commands/readme.txt");
    if !link(&site.at("nowhere"), &place.join("gone.txt")) {
        return;
    }
    let (found, skipped) = list(&place, Kind::File, &site.fence(), |name| {
        name.ends_with(".md")
    });
    assert_eq!(names(&found), ["notes.md"]);
    assert!(skipped.is_empty(), "名字不合的断链接不说：{skipped:?}");
}
