//! 第三方的工具报效果（施工 XG-1，设计 63 第七节第 1 条 A）：扩展登记的 `ext_edit` 声明了哪个参数是要写的路径，回应里只报
//! 哪个文件改了，改前改后由核心自己读。和内置的 `edit` 并排比：两个内容一样的文件，一个交给扩展改、一个交给内置的改，
//! `tool.result` 的效果、`view.detail` 的差异和增删行数一样；接着用内置的 `edit` 改，两边都不用重读；撤掉那一轮，她看过的照
//! 有效历史退回去、文件不动，再改两边一样被拦。撤销不改回文件（施工 U-1），所以不看文件回没回去。

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};

use miyu_kernel::event::{Body, Effect, ToolResult, ToolStatus};
use miyu_session::testkit::{Play, Script};
use miyu_tool::Catalog;

use crate::support::extensions::*;
use crate::support::*;

/// 基础系统的工具：要用内置的 `read`、`edit`。
fn tools() -> Catalog {
    let resources = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources");
    Catalog::new(miyu_basesystem::tools(&resources).expect("出厂的资源读得出来")).expect("合写法")
}

/// 等到 `path` 里记下的有 `n` 行。
async fn lines(path: &Path, n: usize) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    while read(path).lines().count() < n {
        assert!(
            tokio::time::Instant::now() < deadline,
            "等不到 {n} 行：{}",
            read(path)
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

/// 给人看的路径：Windows 上去掉 `\\?\` 这个前缀。
fn plain(path: &Path) -> String {
    let text = path.to_string_lossy();
    text.strip_prefix(r"\\?\").unwrap_or(&text).to_string()
}

/// 改 `file` 的一次调用：内置的 `edit` 或者扩展的 `ext_edit`，参数一样。
fn edit(tool: &str, file: &str, old: &str, new: &str) -> Play {
    let args = json!({"file_path": file, "edits": [{"old_string": old, "new_string": new}]});
    Play::calls(&[(tool, &args.to_string())])
}

/// 日志里的工具结果，照先后。
fn results(home: &Home, session: &str) -> Vec<ToolResult> {
    home.log(session)
        .into_iter()
        .filter_map(|event| match event.body {
            Body::ToolResult(result) => Some(result),
            _ => None,
        })
        .collect()
}

/// 一次结果里的文件改动，路径换成 `-`：两个文件内容一样，别的格要一样。
fn changes(result: &ToolResult) -> Vec<Value> {
    result
        .effects
        .iter()
        .filter_map(|effect| match effect {
            Effect::FileChanged(changed) => {
                let mut changed = serde_json::to_value(changed).expect("写得成");
                changed["path"] = json!("-");
                Some(changed)
            }
            _ => None,
        })
        .collect()
}

/// `view.detail` 里这一次调用改的那个文件：差异、增、删。
async fn detail(client: &mut Client, session: &str, result: &ToolResult) -> Value {
    let reply = client
        .call(
            "d",
            "view.detail",
            json!({"session": session, "call": result.call_id.to_string()}),
        )
        .await;
    let file = &reply["result"]["files"][0];
    json!({"diff": file["diff"], "added": file["added"], "removed": file["removed"]})
}

#[tokio::test]
#[ignore = "红：等 XG-1"]
async fn an_extension_edit_reports_the_same_effects_as_the_builtin_one() {
    let home = Home::new();
    let program = Program::new();
    for name in ["a.txt", "b.txt"] {
        std::fs::write(home.work.join(name), "old\n").expect("写得进");
    }
    let a = std::fs::canonicalize(home.work.join("a.txt")).expect("在");
    let b = std::fs::canonicalize(home.work.join("b.txt")).expect("在");
    let (path, keep) = record(&home, "editor");
    let offered = json!({"tools": [{
        "name": "ext_edit", "description": "Replace exact text in a file.",
        "input_schema": {"type": "object", "properties": {
            "file_path": {"type": "string"}, "edits": {"type": "array"},
        }, "required": ["file_path", "edits"]},
        "access": "write", "venues": ["local"],
        "paths": [{"parameter": "file_path", "write": true}],
    }]});
    install_providing(
        &home,
        "editor",
        &program.name(),
        "always",
        &steps(&[
            &keep,
            &format!("edits:{}", a.display()),
            "hello",
            &format!("ask:provide:{offered}"),
            "serve",
        ]),
    );
    let script = Script::new([
        // 第一轮：两个都读过。
        Play::calls(&[("read", r#"{"file_path":"a.txt"}"#)]),
        Play::calls(&[("read", r#"{"file_path":"b.txt"}"#)]),
        Play::Says("读了。"),
        // 第二轮：一个交给扩展改、一个交给内置的改，再都用内置的接着改。
        edit("ext_edit", "a.txt", "old", "new"),
        edit("edit", "b.txt", "old", "new"),
        edit("edit", "a.txt", "new", "newer"),
        edit("edit", "b.txt", "new", "newer"),
        Play::Says("改了。"),
        // 撤掉第二轮以后：她看过的退回第一轮读的，两边一样被拦。
        edit("edit", "a.txt", "newer", "newest"),
        edit("edit", "b.txt", "newer", "newest"),
        Play::Says("再改。"),
    ]);
    let core = Arc::new(
        home.core_full(&script, tools(), None, TOKEN)
            .with_extension_timing(quick()),
    );
    core.start_extensions();
    // 记下的：工作目录、握手、登记的回应。
    lines(&path, 3).await;
    let registered = read(&path);
    let registered = registered.lines().nth(2).expect("登记的回应");
    assert!(
        registered.contains(r#""result""#),
        "声明了 paths 的登记得上：{registered}"
    );

    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let session = client.create("c1", &home.work.to_string_lossy()).await;
    client.say("c2", &session, "读一下").await;
    home.until_turns(&session, 1).await;
    client.say("c3", &session, "改一下").await;
    home.until_turns(&session, 2).await;

    let done = results(&home, &session);
    assert_eq!(done.len(), 6, "读两次、改四次：{done:?}");
    let (by_extension, by_builtin) = (&done[2], &done[3]);
    assert_eq!(by_extension.status, ToolStatus::Ok, "{by_extension:?}");
    assert_eq!(by_builtin.status, ToolStatus::Ok, "{by_builtin:?}");
    assert_eq!(changes(by_builtin).len(), 1, "{by_builtin:?}");
    assert_eq!(
        changes(by_extension),
        changes(by_builtin),
        "扩展报的改动记成了和内置的一样的 file.changed：{by_extension:?}"
    );
    let changed_path = by_extension.effects.iter().find_map(|effect| match effect {
        Effect::FileChanged(changed) => Some(changed.path.clone()),
        _ => None,
    });
    assert_eq!(changed_path, Some(plain(&a)), "照真实的位置记");
    let called: Value = serde_json::from_str(
        read(&path)
            .lines()
            .find(|line| line.contains(r#""tool":"ext_edit""#))
            .expect("扩展收到了"),
    )
    .expect("一行 JSON");
    assert_eq!(
        called["params"]["checked"],
        json!([plain(&a)]),
        "tool.call 带着链判过的真实位置：{called}"
    );

    let extension_detail = detail(&mut client, &session, by_extension).await;
    let builtin_detail = detail(&mut client, &session, by_builtin).await;
    assert!(builtin_detail["diff"].is_array(), "{builtin_detail}");
    assert_eq!(extension_detail, builtin_detail, "差异、增删行数一样");

    for (then, which) in [(&done[4], "a.txt"), (&done[5], "b.txt")] {
        assert_eq!(
            then.status,
            ToolStatus::Ok,
            "{which}：她看过的是改后的，内置的 edit 接着改不用重读：{then:?}"
        );
    }
    assert_eq!(std::fs::read_to_string(&a).expect("在"), "newer\n");
    assert_eq!(std::fs::read_to_string(&b).expect("在"), "newer\n");

    let reverted = client
        .call("c4", "session.revert", json!({"session": session}))
        .await;
    assert!(reverted.get("error").is_none(), "{reverted}");
    client.say("c5", &session, "再改").await;
    home.until_turns(&session, 3).await;
    let after = results(&home, &session);
    let (a_after, b_after) = (&after[after.len() - 2], &after[after.len() - 1]);
    assert_ne!(
        b_after.status,
        ToolStatus::Ok,
        "撤掉以后她看过的退回第一轮读的，内置改过的被拦：{b_after:?}"
    );
    // 拦下的那句话里有文件名：换成同一个再比。
    let said = |result: &ToolResult| format!("{:?}", result.blocks).replace("a.txt", "b.txt");
    assert_eq!(
        (&a_after.status, said(a_after)),
        (&b_after.status, said(b_after)),
        "扩展改过的和内置改过的一样被拦"
    );
    assert_eq!(
        std::fs::read_to_string(&a).expect("在"),
        "newer\n",
        "文件不动"
    );
    assert_eq!(
        std::fs::read_to_string(&b).expect("在"),
        "newer\n",
        "文件不动"
    );
}
