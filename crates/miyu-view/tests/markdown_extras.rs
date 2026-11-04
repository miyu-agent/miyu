//! 界面上画了、导出里补上的三样（施工 SE-1 中下中）：要人确认的那一次（问了什么、怎么答的）、脚本里调的工具（`nested`）、工具
//! 上挂的提问和回答。`read` 照语言写，`agent` 写原文，`chat` 一行写完；`tools: full` 再把脚本里调的工具交回的结果从 blob 取来。
//! 脚本里调的照真会话跑。

use std::sync::Arc;

use serde_json::json;

use miyu_kernel::id::ModuleId;
use miyu_kernel::session::Verdict;
use miyu_kernel::testkit::{Line, Play, Stage};
use miyu_view::markdown::{Options, Sources, Tools};
use miyu_view::{Entry, Projector};

use crate::markdown::entry;
use crate::markdown_presets::rendered;
use crate::support::*;

/// 一步要确认、被拒的命令，一步问了两题、答了的提问。
fn asked() -> Vec<Entry> {
    vec![
        entry(
            json!({"id": "b4.1", "kind": "tool", "group": "g4.0", "call": "call_5_1", "name": "shell",
            "title": {"name": "执行命令", "object": "rm -rf build"}, "state": "denied",
            "args": "{\"command\":\"rm -rf build\"}", "output": "The person denied this call: 先别删",
            "approval": {"access": "execute", "decision": "deny", "reason": "先别删",
                "by": {"kind": "person"}}}),
        ),
        entry(
            json!({"id": "b4.2", "kind": "tool", "group": "g4.0", "call": "call_5_2", "name": "ask",
            "title": {"name": "提问"}, "state": "ok", "args": "{}", "output": "继续; 写 main.rs",
            "questions": [
                {"question": "要不要继续？", "options": [{"label": "继续"}, {"label": "停下"}]},
                {"question": "先看哪个文件？"},
            ]}),
        ),
        entry(json!({"id": "e8", "kind": "notice", "what": "answered",
            "questions": [{"question": "要不要继续？", "options": [{"label": "继续"}, {"label": "停下"}]},
                {"question": "先看哪个文件？"}],
            "answers": [{"picked": ["继续"]}, {"text": "写 main.rs"}],
            "by": {"kind": "person"}})),
    ]
}

#[test]
fn read_shows_the_approval_and_the_questions_in_the_connection_language() {
    let got = rendered(&asked(), &Sources::default(), Options::READ).text;
    assert!(
        got.contains("### Tool: 执行命令 `rm -rf build`\n\n被拒\n\n要确认（执行命令）：拒绝，理由：先别删\n\n```text"),
        "{got}"
    );
    assert!(
        got.contains("### Tool: 提问\n\n- 要不要继续？ — 继续 / 停下\n- 先看哪个文件？\n\n```text\n继续; 写 main.rs\n```"),
        "{got}"
    );
    assert!(
        got.contains("> 回答了提问\n\n- 要不要继续？: 继续\n- 先看哪个文件？: 写 main.rs"),
        "{got}"
    );
}

#[test]
fn agent_writes_the_approval_raw_and_chat_puts_everything_on_one_line() {
    let agent = rendered(&asked(), &Sources::default(), Options::AGENT).text;
    assert!(
        agent.contains("### Tool: shell (denied)\n\n```json\n{\"command\":\"rm -rf build\"}\n```\n\nApproval (execute): deny — 先别删\n\n```text"),
        "{agent}"
    );
    let chat = rendered(&asked(), &Sources::default(), Options::CHAT).text;
    assert!(
        chat.contains("- Tool: `shell` `rm -rf build` · denied · approval (execute): deny — 先别删\n- Tool: `ask` · asked: 要不要继续？; 先看哪个文件？\n\n> 回答了提问：要不要继续？: 继续; 先看哪个文件？: 写 main.rs"),
        "{chat}"
    );
    let english = {
        let texts = texts("en");
        miyu_view::markdown::render(&miyu_view::markdown::Document {
            entries: &asked(),
            about: &crate::markdown::about(),
            sources: &Sources::default(),
            texts: &texts,
            options: Options::READ,
            offset: miyu_kernel::time::UtcOffset::from_minutes(0).expect("在范围里"),
            secrets: &[],
        })
        .text
    };
    assert!(
        english.contains("Approval (run commands): denied — 先别删"),
        "{english}"
    );
}

/// 她跑一个脚本：脚本里调了三件（成了、被拒、出错），交回整份日志喂出来的条目和原文。
fn scripted() -> (Vec<Entry>, Sources) {
    let mut stage: Stage = stage();
    stage.model([
        Line::calls(
            "",
            &[(
                "shell",
                r#"{"command":"python3 x.py","description":"Count"}"#,
            )],
        ),
        Line::says("ok"),
    ]);
    stage.tools([Play::done("done").held()]);
    stage.say("count");
    let shell = stage.held_tools()[0].to_string();
    stage.nested_call(
        &format!("{shell}#1"),
        "web_fetch",
        r#"{"url":"https://a.example"}"#,
    );
    stage.nested_done(&format!("{shell}#1"), false);
    stage.nested_guards([Verdict::Deny {
        module: ModuleId::parse("permissions").expect("合写法"),
        text: "no".to_string(),
        human: None,
    }]);
    stage.nested_call(
        &format!("{shell}#2"),
        "web_fetch",
        r#"{"url":"https://b.example"}"#,
    );
    stage.nested_call(
        &format!("{shell}#3"),
        "web_fetch",
        r#"{"url":"https://c.example"}"#,
    );
    stage.nested_done(&format!("{shell}#3"), true);
    for call in stage.held_tools() {
        stage.release_tool(call);
    }
    let mut projector = Projector::new(Arc::new(texts("zh")), None);
    let mut sources = Sources::default();
    for event in stage.log() {
        projector.event(event);
        sources.see(event);
    }
    (projector.entries().to_vec(), sources)
}

#[test]
fn nested_calls_are_listed_in_each_style_and_full_brings_their_results() {
    let (entries, mut sources) = scripted();
    let read = rendered(&entries, &sources, Options::READ).text;
    let list = read
        .find("脚本里调了 3 个工具\n- #1 ")
        .expect("read 列出脚本里调的");
    assert!(
        read[list..].contains("\n- #2 ")
            && read[list..].contains(" · 被拒 · no\n- #3 ")
            && read[list..].contains(" · 出错"),
        "{read}"
    );
    let agent = rendered(&entries, &sources, Options::AGENT).text;
    assert!(
        agent.contains(
            "Nested calls:\n- #1 `web_fetch` `{\"url\":\"https://a.example\"}`: ok\n- #2 `web_fetch` `{\"url\":\"https://b.example\"}`: denied — no\n- #3 `web_fetch` `{\"url\":\"https://c.example\"}`: error"
        ),
        "{agent}"
    );
    let chat = rendered(&entries, &sources, Options::CHAT).text;
    assert!(
        chat.contains("· nested: #1 web_fetch, #2 web_fetch (denied), #3 web_fetch (error)"),
        "{chat}"
    );
    let wanted = sources.wanted(&entries);
    assert_eq!(wanted.len(), 1, "两件交回的结果是同一份 blob：{wanted:?}");
    for hash in wanted {
        sources.blobs.insert(hash, "result\n".to_string());
    }
    let full = rendered(
        &entries,
        &sources,
        Options {
            tools: Tools::Full,
            ..Options::READ
        },
    )
    .text;
    assert!(
        full.contains(
            "<details>\n<summary>#1 输出（1 行）</summary>\n\n```text\nresult\n```\n\n</details>"
        ),
        "{full}"
    );
    let full = rendered(
        &entries,
        &sources,
        Options {
            tools: Tools::Full,
            ..Options::AGENT
        },
    )
    .text;
    assert!(
        full.contains("Nested call #3 returned:\n\n```text\nresult\n```"),
        "{full}"
    );
    assert!(
        !rendered(&entries, &sources, Options::READ)
            .text
            .contains("#1 输出"),
        "seen 不取 blob"
    );
}
