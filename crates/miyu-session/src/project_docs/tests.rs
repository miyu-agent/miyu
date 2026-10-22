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
        format!("<p path=\"{}\">\nv1\n</p>\n", path.display())
    );
    assert_eq!(first.module.as_str(), "project-docs");
    let seen = [present(&first)];
    assert_eq!(tail(&texts(), Some(&found), &seen), None, "一样的不交");
    dir.write("AGENTS.md", "v2");
    let changed = find(&dir.0).unwrap();
    let second = tail(&texts(), Some(&changed), &seen).expect("变了的交");
    assert!(
        second
            .fact
            .text
            .starts_with(&format!("Replaces {}.\n", dir.0.display())),
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
