//! 印出来的样子（项目主人 2026-09-28 定的）：撤销、恢复一行行对；撤掉了压缩、清空、停掉了任务的那几句；英文。施工 U-1 起
//! 撤销不再改回文件，不印文件、不说命令改的文件（项目主人 2026-10-15 定：回执不提文件）。

use std::path::MAIN_SEPARATOR_STR;

use serde_json::{Value, json};

use super::*;

/// 根目录下面的一处，`parts` 一段段接上：Windows 上得带盘符才算绝对路径。
fn under(parts: &[&str]) -> String {
    let root = PathBuf::from(if cfg!(windows) { "C:\\" } else { "/" });
    parts
        .iter()
        .fold(root, |path, part| path.join(part))
        .to_string_lossy()
        .into_owned()
}

/// 家目录在根下的 `home/me`，会话的工作目录是 `home/me/proj`。
fn plan(direction: Direction, language: Language) -> UndoPlan {
    UndoPlan {
        direction,
        session: None,
        language,
        home: Some(PathBuf::from(under(&["home", "me"]))),
        color: false,
    }
}

fn printed(result: &Value, plan: &UndoPlan) -> String {
    print::lines(result, plan)
        .iter()
        .map(|line| line.paint(plan.color))
        .collect()
}

/// 照 `direction` 印，中文，不上色。
fn printed_as(result: &Value, direction: Direction) -> String {
    printed(result, &plan(direction, Language::Chinese))
}

/// 定样子时的那个例子：撤了一轮。以前的核心会多交 `files`、`commands`，照样不印。
fn agreed() -> Value {
    json!({
        "events": [14],
        "cwd": under(&["home", "me", "proj"]),
        "turns": 1,
        "said": "把 README 改成中文",
        "commands": 2,
        "files": [{"path": under(&["home", "me", "proj", "a.rs"]), "action": "write", "outcome": "restored"}],
    })
}

/// 读一份样本：`docs/designs/samples/cli/` 下，蓝图里的那一块门禁和同一份比。
fn sample(name: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/designs/samples/cli")
        .join(name);
    std::fs::read_to_string(path).expect("有样本")
}

#[test]
fn the_undo_is_the_sample_of_the_drawing() {
    let screen = printed(&agreed(), &plan(Direction::Undo, Language::Chinese));
    assert_eq!(
        screen.replace(MAIN_SEPARATOR_STR, "/"),
        sample("undo-text.txt")
    );
}

#[test]
fn a_restore_says_what_it_brought_back() {
    let printed = printed(&agreed(), &plan(Direction::Restore, Language::Chinese));
    assert_eq!(printed, "· 恢复「把 README 改成中文」这一轮\n");
}

#[test]
fn in_english_too() {
    let printed = printed(&agreed(), &plan(Direction::Undo, Language::English));
    assert_eq!(
        printed,
        "· Undid the turn \u{201c}把 README 改成中文\u{201d}\nUntil you say something else, miyu restore brings it back.\n"
    );
}

/// 太长的截断：人说的话留前面 40 个字。
#[test]
fn long_words_are_cut() {
    let said = "说".repeat(50);
    let result = json!({"cwd": under(&["w"]), "turns": 1, "said": said});
    let printed = printed_as(&result, Direction::Undo);
    let first = printed.lines().next().expect("有第一行");
    assert_eq!(first, format!("· 撤销「{}…」这一轮", "说".repeat(40)));
}

/// 恢复的是几轮的（一次撤了几轮）：写「起的几轮」。
#[test]
fn several_turns_are_counted() {
    let result = json!({"cwd": under(&["w"]), "turns": 3, "said": "第一句"});
    assert_eq!(
        printed_as(&result, Direction::Restore),
        "· 恢复「第一句」起的 3 轮\n"
    );
    assert!(
        printed_as(&result, Direction::Undo).starts_with("· 撤销「第一句」起的 3 轮\n"),
        "撤销的也一样"
    );
}

#[test]
fn every_first_line_of_a_restore_names_it_restore() {
    // 施工 4-7 补改名：恢复的第一行照新名字写，英文是 Restored，没有 Redid。
    let english = Language::English;
    assert_eq!(
        english.undo_header(Direction::Restore, Some("hi"), 1),
        "· Restored the turn \u{201c}hi\u{201d}"
    );
    assert_eq!(
        english.undo_header(Direction::Restore, Some("hi"), 3),
        "· Restored 3 turns from \u{201c}hi\u{201d}"
    );
    assert_eq!(
        english.undo_header(Direction::Restore, None, 1),
        "· Restored the undone turn"
    );
    assert_eq!(
        Language::Chinese.undo_header(Direction::Restore, None, 1),
        "· 恢复撤销的那一轮"
    );
}

/// 撤掉了压缩（施工 6-9）：第一行下面说一句，撤掉一次、几次都是这一句（2026-09-29 项目主人定）；是 0 的、没有这一格的、
/// 恢复的不说。
#[test]
fn an_undone_compaction_is_said_once_below_the_first_line() {
    let result = |compactions: u64| json!({"cwd": under(&["w"]), "turns": 1, "said": "接着来", "compactions": compactions});
    let once = "· 撤销「接着来」这一轮\n· 撤掉了压缩，上下文回到了压缩前\n发下一句之前，可以用 miyu restore 恢复。\n";
    assert_eq!(printed_as(&result(1), Direction::Undo), once);
    assert_eq!(
        printed_as(&result(2), Direction::Undo),
        once,
        "几次都是同一句"
    );
    assert!(!printed_as(&result(0), Direction::Undo).contains("压缩"));
    let mut without = result(1);
    without.as_object_mut().map(|map| map.remove("compactions"));
    assert!(!printed_as(&without, Direction::Undo).contains("压缩"));
    assert!(!printed_as(&result(1), Direction::Restore).contains("压缩"));
    let english = printed(&result(3), &plan(Direction::Undo, Language::English));
    assert!(
        english.contains("\n· Undid the compaction; the context is back to how it was before\n"),
        "{english}"
    );
}

/// 撤掉了清空（施工 6-8 补）：一次、几次都是这一句（2026-09-30 项目主人定），和撤掉了压缩那一句都有的，先压缩后清空；是 0
/// 的、没有这一格的、恢复的不说。两样都有的样子和样本 `undo-clear-text.txt` 逐字节一样。
#[test]
fn an_undone_clear_is_said_after_an_undone_compaction() {
    let result = |compactions: u64, clears: u64| {
        json!({"cwd": under(&["home", "me", "proj"]), "turns": 3, "said": "接着把测试补完",
               "compactions": compactions, "clears": clears})
    };
    let chinese = plan(Direction::Undo, Language::Chinese);
    assert_eq!(
        printed(&result(1, 2), &chinese),
        sample("undo-clear-text.txt")
    );
    let alone = printed(&result(0, 1), &chinese);
    assert!(
        alone.contains("起的 3 轮\n· 撤掉了清空，上下文回到了清空以前\n"),
        "{alone}"
    );
    assert!(!alone.contains("压缩"), "{alone}");
    assert!(!printed(&result(1, 0), &chinese).contains("清空"));
    let mut without = result(0, 1);
    without.as_object_mut().map(|map| map.remove("clears"));
    assert!(!printed(&without, &chinese).contains("清空"));
    assert!(!printed(&result(1, 1), &plan(Direction::Restore, Language::Chinese)).contains("清空"));
    let english = printed(&result(1, 1), &plan(Direction::Undo, Language::English));
    assert!(
        english.contains(
            "\n· Undid the compaction; the context is back to how it was before\n· Undid the clear; the context is back to before it.\n"
        ),
        "{english}"
    );
}

/// 停掉了那几轮派出去的任务（施工 7-8）：说一句停掉了几个，在撤掉了压缩、清空的那两句下面；没有的、空的、恢复的不说；英文
/// 一个写单数。样子和样本 `undo-jobs-text.txt` 逐字节一样。
#[test]
fn stopped_jobs_are_said_in_one_line() {
    let result = |jobs: Option<Value>| {
        let mut result = json!({"cwd": under(&["home", "me", "proj"]), "turns": 1,
                                "said": "后台跑测试，再派一个去查 CI"});
        if let Some(jobs) = jobs {
            result["jobs"] = jobs;
        }
        result
    };
    let one = json!([{"job": "j1", "what": "command", "title": "跑测试"}]);
    let two = json!([
        {"job": "j1", "what": "command", "title": "跑测试"},
        {"job": "j2", "what": "agent", "title": "查 CI"},
    ]);
    let chinese = plan(Direction::Undo, Language::Chinese);
    assert_eq!(
        printed(&result(Some(two.clone())), &chinese),
        sample("undo-jobs-text.txt")
    );
    for quiet in [None, Some(json!([]))] {
        assert!(!printed(&result(quiet), &chinese).contains("任务"));
    }
    let restore = plan(Direction::Restore, Language::Chinese);
    assert!(!printed(&result(Some(two.clone())), &restore).contains("任务"));
    let english = plan(Direction::Undo, Language::English);
    assert!(printed(&result(Some(one)), &english).contains("\n· Stopped 1 job\n"));
    assert!(printed(&result(Some(two)), &english).contains("\n· Stopped 2 jobs\n"));
}
