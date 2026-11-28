//! 项目指令的测试：仓库里从根找到工作目录、先 AGENTS.md 后 CLAUDE.md；仓库外只找工作目录那一层；总上限截断；交过的一样的
//! 不交、变了的换一份、别的仓库的另算。

use super::*;

fn texts() -> ProjectDocTexts {
    ProjectDocTexts {
        open: "<p path=\"{path}\">\n".to_string(),
        close: "</p>\n".to_string(),
        replaced: "Replaces {root}.\n".to_string(),
        cut: "Cut at {limit}.\n".to_string(),
    }
}

/// 一个临时目录，测完删掉。
struct Dir(PathBuf);

impl Dir {
    fn new(name: &str) -> Dir {
        let path = std::env::temp_dir().join(format!("miyu-docs-{name}-{}", std::process::id()));
        drop(std::fs::remove_dir_all(&path));
        std::fs::create_dir_all(&path).unwrap();
        Dir(path.canonicalize().unwrap())
    }

    fn write(&self, rel: &str, text: &str) -> PathBuf {
        let path = self.0.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, text).unwrap();
        path
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        drop(std::fs::remove_dir_all(&self.0));
    }
}

fn present(injected: &Injection) -> Present {
    Present {
        module: injected.module.clone(),
        kind: injected.fact.kind.clone(),
        refs: injected.fact.refs.clone(),
    }
}

#[test]
fn in_a_repo_every_layer_from_the_root_down_to_the_cwd() {
    let dir = Dir::new("repo");
    std::fs::create_dir_all(dir.0.join("repo/.git")).unwrap();
    let root = dir.write("repo/AGENTS.md", "root rules\n");
    dir.write("repo/CLAUDE.md", "not this one\n");
    let sub = dir.write("repo/a/CLAUDE.md", "a rules\n");
    dir.write("repo/a/b/README.md", "no doc here\n");
    dir.write("AGENTS.md", "outside the repo\n");
    let found = find(&dir.0.join("repo/a/b")).unwrap();
    assert_eq!(found.root, dir.0.join("repo"));
    assert_eq!(
        found.docs,
        [
            (root, "root rules\n".to_string()),
            (sub, "a rules\n".to_string())
        ]
    );
    assert!(!found.cut);
}

#[test]
fn outside_a_repo_only_the_cwd_itself() {
    let dir = Dir::new("plain");
    dir.write("AGENTS.md", "parent\n");
    let own = dir.write("notes/AGENTS.md", "own\n");
    let found = find(&dir.0.join("notes")).unwrap();
    assert_eq!(found.docs, [(own, "own\n".to_string())]);
    assert!(find(&dir.0.join("empty")).is_none(), "没有的不交");
}

#[test]
fn all_layers_share_one_limit() {
    let dir = Dir::new("limit");
    std::fs::create_dir_all(dir.0.join(".git")).unwrap();
    dir.write("AGENTS.md", &"a".repeat(LIMIT - 10));
    dir.write("sub/AGENTS.md", &"é".repeat(20));
    let found = find(&dir.0.join("sub")).unwrap();
    assert!(found.cut);
    assert_eq!(found.docs[1].1, "é".repeat(5), "不切开一个字");
    let text = tail(&texts(), Some(&found), &[]).unwrap().fact.text;
    assert!(text.ends_with(&format!("Cut at {LIMIT}.\n")), "{text}");
}

#[test]
fn told_once_replaced_when_changed_and_per_repo() {
    let dir = Dir::new("tell");
    std::fs::create_dir_all(dir.0.join(".git")).unwrap();
    let path = dir.write("AGENTS.md", "v1");
    let found = find(&dir.0).unwrap();
    let first = tail(&texts(), Some(&found), &[]).expect("没交过的交");
    assert_eq!(
        first.fact.text,
        format!(
            "<p path=\"{}\">\nv1\n</p>\n",
            crate::path_text::path_text(&path)
        )
    );
    assert_eq!(first.module.as_str(), "project-docs");
    let seen = [present(&first)];
    assert_eq!(tail(&texts(), Some(&found), &seen), None, "一样的不交");
    dir.write("AGENTS.md", "v2");
    let changed = find(&dir.0).unwrap();
    let second = tail(&texts(), Some(&changed), &seen).expect("变了的交");
    assert!(
        second.fact.text.starts_with(&format!(
            "Replaces {}.\n",
            crate::path_text::path_text(&dir.0)
        )),
        "{}",
        second.fact.text
    );
    // 别的仓库交过的不算这一个的。
    let other = Present {
        refs: vec![first.fact.refs[0].clone(), "/elsewhere".to_string()],
        ..present(&first)
    };
    assert!(tail(&texts(), Some(&found), &[other]).is_some());
    assert_eq!(tail(&texts(), None, &[]), None);
}

#[test]
fn the_shipped_texts_fill_in() {
    let root = miyu_store::resources::ResourceRoot::at(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources"),
    );
    let texts = root.project_doc_texts().expect("出厂的字都在");
    check(&texts).expect("字段都对");
}

/// 子目录的（施工 P-14 下）：工作目录下面、到文件所在目录为止的每一层，先 `AGENTS.md` 后 `CLAUDE.md`，从外到里；工作目录
/// 那一层和以上不算；不在工作目录下面的没有。
#[test]
fn below_takes_the_layers_under_the_cwd_down_to_the_file() {
    let dir = Dir::new("below");
    dir.write("AGENTS.md", "cwd\n");
    dir.write("sub/CLAUDE.md", "sub claude\n");
    dir.write("sub/deep/AGENTS.md", "deep agents\n");
    dir.write("sub/deep/CLAUDE.md", "deep claude\n");
    let file = dir.write("sub/deep/a.txt", "x\n");
    let texts: Vec<String> = below(&dir.0, &file)
        .into_iter()
        .map(|(_, text)| text)
        .collect();
    assert_eq!(texts, ["sub claude\n", "deep agents\n"]);
    let here = dir.write("b.txt", "x\n");
    assert!(
        below(&dir.0, &here).is_empty(),
        "工作目录那一层回合开始交过"
    );
    let outside = Dir::new("below-out");
    let elsewhere = outside.write("x/a.txt", "x\n");
    outside.write("x/AGENTS.md", "no\n");
    assert!(
        below(&dir.0, &elsewhere).is_empty(),
        "不在工作目录下面的没有"
    );
}

/// 照整份读来算的：上限那里切开一个字、两层分一个上限，有界地读交回的字、截断的位置和整份读完再截的一模一样（施工 AUD-7）。
#[test]
fn a_bounded_read_cuts_where_reading_it_all_would() {
    for pad in 0..8 {
        for outer in [0, 1000] {
            let dir = Dir::new(&format!("bounded-{pad}-{outer}"));
            std::fs::create_dir_all(dir.0.join(".git")).unwrap();
            let inner = format!("{}{}tail\n", "a".repeat(LIMIT - 4 + pad), "😀".repeat(3));
            let mut expected = Vec::new();
            let mut left = LIMIT;
            if outer > 0 {
                let path = dir.write("AGENTS.md", &"o".repeat(outer));
                expected.push((path, "o".repeat(outer)));
                left -= outer;
            }
            let path = dir.write("sub/AGENTS.md", &inner);
            expected.push((path, cut_at(&inner, left).to_string()));
            let found = find(&dir.0.join("sub")).unwrap();
            assert_eq!(found.docs, expected, "pad {pad} outer {outer}");
            assert!(found.cut);
        }
    }
    // 正好是上限、比上限多一个字的：照旧整份交、截一个字。
    let dir = Dir::new("bounded-edge");
    dir.write("AGENTS.md", &"a".repeat(LIMIT));
    let found = find(&dir.0).unwrap();
    assert_eq!((found.docs[0].1.len(), found.cut), (LIMIT, false));
    dir.write("AGENTS.md", &"a".repeat(LIMIT + 1));
    let found = find(&dir.0).unwrap();
    assert_eq!((found.docs[0].1.len(), found.cut), (LIMIT, true));
    // 结尾坏了半个字、中间不是 UTF-8 的：照旧当没有，找 CLAUDE.md。
    for broken in [&b"rules\n\xF0\x9F"[..], &b"ru\xFFles\n"[..]] {
        std::fs::write(dir.0.join("AGENTS.md"), broken).unwrap();
        dir.write("CLAUDE.md", "claude\n");
        let found = find(&dir.0).unwrap();
        assert_eq!(found.docs[0].1, "claude\n");
    }
}

/// 仓库根、子目录里的 `AGENTS.md` 都是指向 `target` 的链接（施工 AUD-7）：当没有，找 CLAUDE.md，十秒内交回。
#[cfg(unix)]
fn linked_doc_is_skipped(name: &str, target: &Path) {
    let repo = Dir::new(name);
    std::fs::create_dir_all(repo.0.join("sub")).unwrap();
    std::os::unix::fs::symlink(target, repo.0.join("AGENTS.md")).unwrap();
    std::fs::write(repo.0.join("CLAUDE.md"), "claude\n").unwrap();
    std::os::unix::fs::symlink(target, repo.0.join("sub/AGENTS.md")).unwrap();
    std::fs::write(repo.0.join("sub/a.txt"), "x\n").unwrap();
    let (send, receive) = std::sync::mpsc::channel();
    let looked = repo.0.clone();
    std::thread::spawn(move || {
        let found = find(&looked).map(|found| found.docs);
        let below = below(&looked, &looked.join("sub/a.txt"));
        drop(send.send((found, below)));
    });
    let (found, below) = receive
        .recv_timeout(std::time::Duration::from_secs(10))
        .unwrap_or_else(|_| panic!("{name}: 卡住了"));
    assert_eq!(
        found,
        Some(vec![(repo.0.join("CLAUDE.md"), "claude\n".to_string())]),
        "{name}"
    );
    assert!(below.is_empty(), "{name}: {below:?}");
}

/// 指向 `/dev/zero` 的：修之前整份往内存里读，读到内存吃光。
#[cfg(unix)]
#[test]
fn a_doc_linked_to_dev_zero_is_skipped() {
    linked_doc_is_skipped("zero", Path::new("/dev/zero"));
}

/// 指向 FIFO 的：修之前打开时一直等人来写，卡住。
#[cfg(unix)]
#[test]
fn a_doc_linked_to_a_fifo_is_skipped() {
    let dir = Dir::new("fifo-target");
    let fifo = dir.0.join("fifo");
    let made = std::process::Command::new("mkfifo")
        .arg(&fifo)
        .status()
        .unwrap();
    assert!(made.success());
    linked_doc_is_skipped("fifo", &fifo);
}

/// 指向普通文件的链接照常读（`CLAUDE.md -> AGENTS.md` 是常见的写法）。
#[cfg(unix)]
#[test]
fn a_doc_linked_to_a_regular_file_is_read() {
    let dir = Dir::new("linked");
    std::fs::write(dir.0.join("rules.md"), "rules\n").unwrap();
    std::os::unix::fs::symlink(dir.0.join("rules.md"), dir.0.join("CLAUDE.md")).unwrap();
    let found = find(&dir.0).unwrap();
    assert_eq!(
        found.docs,
        vec![(dir.0.join("CLAUDE.md"), "rules\n".to_string())]
    );
}
