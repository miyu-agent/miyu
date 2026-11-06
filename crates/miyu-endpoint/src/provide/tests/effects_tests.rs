//! 扩展交的效果（施工 XG-1 上，设计 `63-扩展工具的效果和声明.md` 第四节）：只收过了链的路径，改前改后、读的哈希都是核心自己读的；
//! 说了没改的、对不上的、太大的、不收的种类、写法不对的各自丢掉，别的照收。丢的那一行（`WARN tool effect dropped`）由
//! `tests/provide_effects_log.rs` 用真核心看。

use std::path::{Path, PathBuf};

use miyu_kernel::id::ContentHash;
use miyu_tool::Effect;

use super::super::effects::{Befores, Content, Context, Judged, vet};
use super::*;

/// 一个临时的工作区：真实位置，丢掉时删掉。
fn workspace() -> (PathBuf, crate::test_support::TempDir) {
    let (_, dir) = crate::test_support::temp_root("effects");
    let work = dir.join("work");
    std::fs::create_dir_all(&work).expect("建得了");
    (std::fs::canonicalize(&work).expect("在"), dir)
}

/// 照 `judged`、`befores` 查 `effects`，工作目录是 `cwd`；`error` 是回应说出了错。
fn vetted(
    cwd: &Path,
    judged: &[Judged],
    befores: &Befores,
    effects: Value,
    error: bool,
) -> Vec<Effect> {
    let cwd = cwd.to_string_lossy();
    vet(
        Some(effects),
        &Context {
            tool: "ext_edit",
            cwd: &cwd,
            home: None,
            judged,
            befores,
            error,
        },
    )
}

/// 判过的一条。
fn judged(real: &Path, write: bool) -> Judged {
    Judged {
        real: real.to_path_buf(),
        write,
    }
}

/// 改了的：改前是发之前留的那份，改后由核心现读，报的哈希、原文对得上才收；说改了却没改的、对不上的丢。相对的路径照工作
/// 目录换，记真实位置。
#[test]
fn a_change_is_read_by_the_core_before_and_after() {
    let (work, _dir) = workspace();
    let a = work.join("a.txt");
    std::fs::write(&a, "new\n").expect("写得进");
    let judged = [judged(&a, true)];
    let befores = Befores::from([(a.clone(), Content::Bytes(b"old\n".to_vec()))]);
    let changed = Effect::Changed {
        path: a.clone(),
        before: Some(b"old\n".to_vec()),
        after: b"new\n".to_vec(),
    };
    let hash = |text: &str| ContentHash::of(text.as_bytes()).to_string();
    for fine in [
        json!([{"kind": "file.changed", "path": "a.txt"}]),
        json!([{"kind": "file.changed", "path": a, "before": hash("old\n"), "after": hash("new\n"),
                "before_text": "old\n", "after_text": "new\n"}]),
    ] {
        assert_eq!(
            vetted(&work, &judged, &befores, fine.clone(), false),
            std::slice::from_ref(&changed),
            "{fine}"
        );
    }
    for wrong in [
        json!([{"kind": "file.changed", "path": "a.txt", "after": hash("other\n")}]),
        json!([{"kind": "file.changed", "path": "a.txt", "before": hash("other\n")}]),
        json!([{"kind": "file.changed", "path": "a.txt", "after_text": "newer\n"}]),
        json!([{"kind": "file.changed", "path": "a.txt", "before_text": "older\n"}]),
    ] {
        assert!(
            vetted(&work, &judged, &befores, wrong.clone(), false).is_empty(),
            "对不上的丢：{wrong}"
        );
    }
    // 改前改后一样：没改。
    let same = Befores::from([(a.clone(), Content::Bytes(b"new\n".to_vec()))]);
    let said = json!([{"kind": "file.changed", "path": "a.txt"}]);
    assert!(vetted(&work, &judged, &same, said.clone(), false).is_empty());
    // 新建的：改前没有；新建的却说了改前的，对不上。
    let created = Befores::from([(a.clone(), Content::Missing)]);
    assert_eq!(
        vetted(&work, &judged, &created, said.clone(), false),
        [Effect::Changed {
            path: a.clone(),
            before: None,
            after: b"new\n".to_vec()
        }]
    );
    let claimed = json!([{"kind": "file.changed", "path": "a.txt", "before": hash("old\n")}]);
    assert!(vetted(&work, &judged, &created, claimed, false).is_empty());
    // 太大的不读；改后读不了的（不在了）对不上。
    let big = Befores::from([(a.clone(), Content::TooBig)]);
    assert!(vetted(&work, &judged, &big, said.clone(), false).is_empty());
    std::fs::remove_file(&a).expect("删得了");
    assert!(vetted(&work, &judged, &befores, said, false).is_empty());
}

/// 只收这次调用过了链的：没判过的（工作区别的文件、`~/.ssh` 那种）核心不去读；只判了读的不收改、删；没声明 `paths` 的（判过的
/// 是空的）一条都不收。
#[test]
fn only_paths_the_chain_judged_are_taken() {
    let (work, _dir) = workspace();
    let (a, other, secret) = (work.join("a.txt"), work.join("b.txt"), work.join("id_rsa"));
    for file in [&a, &other, &secret] {
        std::fs::write(file, "x\n").expect("写得进");
    }
    let befores = Befores::from([(a.clone(), Content::Bytes(b"old\n".to_vec()))]);
    let reported = json!([
        {"kind": "file.changed", "path": other},
        {"kind": "file.read", "path": secret},
        {"kind": "file.trashed", "path": "b.txt", "trash": "trash:b"},
    ]);
    assert!(vetted(&work, &[judged(&a, true)], &befores, reported, false).is_empty());
    let read_only = [judged(&a, false)];
    let writes = json!([
        {"kind": "file.changed", "path": "a.txt"},
        {"kind": "file.trashed", "path": "a.txt", "trash": "trash:a"},
    ]);
    assert!(vetted(&work, &read_only, &befores, writes, false).is_empty());
    let read = json!([{"kind": "file.read", "path": "a.txt", "lines": [1, 1]}]);
    assert_eq!(
        vetted(&work, &read_only, &Befores::new(), read.clone(), false),
        [Effect::Read {
            path: a.clone(),
            lines: Some([1, 1]),
            hash: ContentHash::of(b"x\n")
        }],
        "读的哈希照现读的整份"
    );
    assert!(vetted(&work, &[], &Befores::new(), read, false).is_empty());
}

/// 删了的要真不在了；回收站的位置至多 4096 个字。
#[test]
fn a_trashed_file_must_be_gone() {
    let (work, _dir) = workspace();
    let a = work.join("a.txt");
    std::fs::write(&a, "x\n").expect("写得进");
    let judged = [judged(&a, true)];
    let trashed = json!([{"kind": "file.trashed", "path": "a.txt", "trash": "trash:a"}]);
    assert!(vetted(&work, &judged, &Befores::new(), trashed.clone(), false).is_empty());
    std::fs::remove_file(&a).expect("删得了");
    assert_eq!(
        vetted(&work, &judged, &Befores::new(), trashed, false),
        [Effect::Trashed {
            path: a.clone(),
            trash: "trash:a".to_string()
        }]
    );
    let long = json!([{"kind": "file.trashed", "path": "a.txt", "trash": "t".repeat(4097)}]);
    assert!(vetted(&work, &judged, &Befores::new(), long, false).is_empty());
}

/// 不收的种类整条丢；写法不对的丢；待办只查写法，出错的不收，文件的照收；同一种同一个文件只收第一条。
#[test]
fn other_kinds_and_bad_shapes_are_dropped() {
    let (work, _dir) = workspace();
    let a = work.join("a.txt");
    std::fs::write(&a, "x\n").expect("写得进");
    let judged = [judged(&a, false)];
    let reported = json!([
        {"kind": "job.started", "job": "j1", "what": "command", "title": "x"},
        {"kind": "file.shown", "path": a},
        {"kind": "package.state", "package": "p", "state": {}},
        {"kind": "peer.watch", "session": "x"},
        {"kind": "telepathy"},
        {"path": a},
        "file.read",
        {"kind": "file.read"},
        {"kind": "file.read", "path": "a.txt", "lines": [3, 1]},
        {"kind": "todo.written", "todos": "all"},
        {"kind": "todo.written", "todos": [{"content": "Ship", "status": "pending"}]},
        {"kind": "file.read", "path": "a.txt"},
        {"kind": "file.read", "path": a},
    ]);
    let kept = vetted(&work, &judged, &Befores::new(), reported.clone(), false);
    assert_eq!(kept.len(), 2, "{kept:?}");
    assert!(matches!(&kept[0], Effect::TodoWritten(written) if written.todos.len() == 1));
    assert!(matches!(&kept[1], Effect::Read { path, lines: None, .. } if *path == a));
    let failed = vetted(&work, &judged, &Befores::new(), reported, true);
    assert_eq!(failed.len(), 1, "出错的不收待办：{failed:?}");
    assert!(matches!(&failed[0], Effect::Read { .. }));
    // `effects` 不是数组的整份不收；`null` 当没写。
    for whole in [json!({"kind": "file.read"}), Value::Null] {
        assert!(vetted(&work, &judged, &Befores::new(), whole, false).is_empty());
    }
}

/// 待办的 `done` 写 `null` 的当没写，照收（施工 XG-1 补，同 `effects`、`state` 的 `null`）；写了的照收；写成别的样子的整条丢。
#[test]
fn a_null_done_on_todos_is_unwritten() {
    let (work, _dir) = workspace();
    let todos = json!([{"content": "Ship", "status": "completed"}]);
    let reported = json!([
        {"kind": "todo.written", "todos": [], "done": null},
        {"kind": "todo.written", "todos": [], "done": todos},
        {"kind": "todo.written", "todos": [], "done": "all"},
        {"kind": "todo.written", "todos": [], "done": {}},
        {"kind": "todo.written", "todos": null},
    ]);
    let kept = vetted(&work, &[], &Befores::new(), reported, false);
    assert_eq!(kept.len(), 2, "{kept:?}");
    assert!(matches!(&kept[0], Effect::TodoWritten(written) if written.done.is_empty()));
    assert!(matches!(&kept[1], Effect::TodoWritten(written) if written.done.len() == 1));
}
