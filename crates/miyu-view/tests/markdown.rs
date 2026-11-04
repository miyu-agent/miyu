//! 会话导出成 Markdown 的 `read` 预设（施工 SE-1 中下上，`miyu_view::markdown`）：开头、英文的角色小标题、思考折起来、工具的
//! 显示名和摘要、长输出折起来、附件占位、子代理的交代和回报、压缩的旁白和摘要；撤掉的、撤回的不进；代码块不被正文弄断；
//! 家目录写成 `~`；配了的密钥换掉、报几处。条目照 JSON 造（同头收到的样子），撤销、压缩另照真会话跑一遍。

use std::sync::Arc;

use serde_json::{Value, json};

use miyu_kernel::id::{CallId, Seq};
use miyu_kernel::testkit::Line;
use miyu_kernel::time::{Timestamp, UtcOffset};
use miyu_view::markdown::{About, Document, Options, Rendered, Sources, render};
use miyu_view::{Entry, EntryId, Projector};

use crate::support::*;

/// 一条条目：`at` 没写的照 2026-11-04 01:00 UTC。
fn entry(mut value: Value) -> Entry {
    if value.get("at").is_none() {
        value["at"] = json!("2026-11-04T01:00:00.000Z");
    }
    serde_json::from_value(value.clone()).unwrap_or_else(|e| panic!("{e}: {value}"))
}

fn about() -> About {
    About {
        title: "看看 main.rs".to_string(),
        from: Timestamp::parse("2026-11-04T01:00:00.000Z").expect("合写法"),
        to: Timestamp::parse("2026-11-04T01:32:00.000Z").expect("合写法"),
        tokens: 1234,
    }
}

/// 照中文渲染，时区是东九区。
fn zh(entries: &[Entry], sources: &Sources, secrets: &[(&str, &str)]) -> Rendered {
    let texts = texts("zh");
    render(&Document {
        entries,
        about: &about(),
        sources,
        texts: &texts,
        options: Options::READ,
        offset: UtcOffset::from_minutes(540).expect("在范围里"),
        secrets,
    })
}

fn text(entries: &[Entry]) -> String {
    zh(entries, &Sources::default(), &[]).text
}

/// 一轮：问、想、读文件、答、收尾。
fn one_turn() -> Vec<Entry> {
    vec![
        entry(json!({"id": "m2", "kind": "user", "text": "帮我看看 main.rs"})),
        entry(
            json!({"id": "g4.0", "kind": "group", "steps": ["b4.0", "b4.1"], "summary": [], "summary_en": []}),
        ),
        entry(
            json!({"id": "b4.0", "kind": "thought", "group": "g4.0", "text": "先读一下。", "took_ms": 2000}),
        ),
        entry(
            json!({"id": "b4.1", "kind": "tool", "group": "g4.0", "call": "call_4_1", "name": "read",
            "title": {"name": "读取", "object": "/home/alice/src/main.rs", "said": "读了 3 行"},
            "state": "ok", "args": "{}", "output": "fn main() {\n    println!(\"hi\");\n}"}),
        ),
        entry(json!({"id": "b6.0", "kind": "reply", "text": "就是打一句 `hi`。", "persona": null})),
        entry(
            json!({"id": "e7", "kind": "end", "reason": "completed", "took_ms": 3000,
            "endpoint": "deepseek", "model": "deepseek-chat"}),
        ),
    ]
}

#[test]
fn a_turn_reads_with_english_roles_and_folded_thinking() {
    let markdown = text(&one_turn());
    let expected = "\
# 看看 main.rs

- 时间：2026-11-04 10:00 – 10:32 (UTC+09:00)
- 模型：deepseek/deepseek-chat
- token 合计：1234

## User

帮我看看 main.rs

## Assistant

<details>
<summary>思考了 2s</summary>

先读一下。

</details>

### Tool: 读取 `~/src/main.rs`

读了 3 行

```text
fn main() {
    println!(\"hi\");
}
```

就是打一句 `hi`。
";
    assert_eq!(markdown, expected, "{markdown}");
}

#[test]
fn undone_turns_withdrawn_messages_and_the_undo_notice_stay_out() {
    let mut entries = one_turn();
    entries.push(entry(
        json!({"id": "m8", "kind": "user", "text": "撤掉的那句", "hidden": true}),
    ));
    entries.push(entry(json!({"id": "b9.0", "kind": "reply", "text": "撤掉的回答", "persona": null, "hidden": true})));
    entries.push(entry(json!({"id": "e10", "kind": "notice", "what": "reverted", "turns": [], "said": "撤掉的那句"})));
    entries.push(entry(json!({"id": "m11", "kind": "user", "text": "退回的排队消息", "queued": true, "withdrawn": true})));
    let markdown = text(&entries);
    for gone in ["撤掉的", "退回的排队消息"] {
        assert!(!markdown.contains(gone), "{gone} 不进：{markdown}");
    }
}

#[test]
fn a_real_undo_leaves_only_what_is_still_there() {
    let mut stage = stage();
    stage.model([Line::says("one"), Line::says("two")]);
    stage.say("first");
    stage.say("second");
    let second = stage.turns()[1];
    stage.revert(second);
    let markdown = text(&page(&stage));
    assert!(
        markdown.contains("first") && markdown.contains("one"),
        "{markdown}"
    );
    assert!(
        !markdown.contains("second") && !markdown.contains("two"),
        "{markdown}"
    );
}

#[test]
fn a_real_compaction_sits_on_the_timeline_with_its_summary_folded() {
    let mut stage = stage();
    stage.limits(Some(200_000), Some(8_000));
    stage.model([Line::says("one"), Line::says("two")]);
    stage.say("first");
    summarizes(&mut stage, "summary of the talk");
    stage.request_compaction(None);
    stage.say("second");
    let mut projector = Projector::new(Arc::new(texts("zh")), None);
    let mut sources = Sources::default();
    for event in stage.log() {
        projector.event(event);
        sources.see(event);
    }
    let markdown = zh(projector.entries(), &sources, &[]).text;
    let first = markdown.find("first").expect("有第一句");
    let notice = markdown.find("> 上下文压缩了").expect("有压缩的旁白");
    let second = markdown.find("second").expect("有第二句");
    assert!(first < notice && notice < second, "照时间线：{markdown}");
    assert!(
        markdown
            .contains("<details>\n<summary>摘要</summary>\n\nsummary of the talk\n\n</details>"),
        "{markdown}"
    );
    assert_eq!(
        markdown.matches("## Assistant").count(),
        2,
        "旁白以后她再开口重起小标题：{markdown}"
    );
}

#[test]
fn a_long_output_is_folded_and_a_cut_one_comes_whole_from_the_log() {
    let call = CallId::new(Seq::new(4).expect("非零"), 1).expect("从 1 数起");
    let lines: Vec<String> = (1..=30).map(|n| format!("line {n}")).collect();
    let mut sources = Sources::default();
    sources.outputs.insert(call, lines.join("\n"));
    let entries = vec![entry(
        json!({"id": "b4.1", "kind": "tool", "group": "g4.0", "call": "call_4_1",
        "name": "shell", "title": {"name": "执行命令", "object": "seq 30"}, "state": "error",
        "args": "{}", "output": "line 1\nline 2", "output_cut": true}),
    )];
    let markdown = zh(&entries, &sources, &[]).text;
    assert!(
        markdown.contains("### Tool: 执行命令 `seq 30`\n\n出错"),
        "{markdown}"
    );
    assert!(
        markdown.contains("<details>\n<summary>输出（30 行）</summary>\n\n```text\nline 1\n"),
        "{markdown}"
    );
    assert!(
        markdown.contains("line 30\n```\n\n</details>"),
        "{markdown}"
    );
}

#[test]
fn backticks_in_the_text_never_break_a_code_block() {
    let entries = vec![
        entry(json!({"id": "m2", "kind": "user", "text": "看这段：\n```rust\nfn main() {}\n"})),
        entry(
            json!({"id": "b4.1", "kind": "tool", "group": "g4.0", "call": "call_4_1", "name": "read",
            "title": {"name": "读取", "object": "a `b` c"}, "state": "ok", "args": "{}",
            "output": "前面\n```\n里面\n````\n后面"}),
        ),
        entry(json!({"id": "b6.0", "kind": "reply", "persona": null,
            "text": "图：\n\n```mermaid\ngraph TD\n  A --> B\n```"})),
    ];
    let markdown = text(&entries);
    assert!(
        markdown.contains("```rust\nfn main() {}\n```\n\n## Assistant"),
        "没合上的代码块替它合上：{markdown}"
    );
    assert!(
        markdown.contains("``a `b` c``"),
        "行内代码多一个反引号：{markdown}"
    );
    assert!(
        markdown.contains("`````text\n前面\n```\n里面\n````\n后面\n`````"),
        "围栏比里面最长的一串长：{markdown}"
    );
    assert!(
        markdown.contains("```mermaid\ngraph TD\n  A --> B\n```"),
        "mermaid 留源码：{markdown}"
    );
}

#[test]
fn a_closing_details_tag_in_thinking_does_not_end_the_fold() {
    let entries = vec![entry(
        json!({"id": "b4.0", "kind": "thought", "group": "g4.0",
        "text": "外面 </details> 一下\n```\n</details>\n```"}),
    )];
    let markdown = text(&entries);
    assert!(markdown.contains("外面 &lt;/details> 一下"), "{markdown}");
    assert!(
        markdown.contains("```\n</details>\n```"),
        "代码块里的原样：{markdown}"
    );
    assert!(
        markdown.contains("<summary>思考</summary>"),
        "没记下用时的只写思考：{markdown}"
    );
}

#[test]
fn a_subagent_shows_its_brief_and_its_report() {
    let entries = vec![
        entry(
            json!({"id": "b4.1", "kind": "tool", "group": "g4.0", "call": "call_4_1", "name": "subagent",
            "title": {"name": "子代理", "object": "查依赖"}, "state": "ok", "job": "j1",
            "args": "{\"description\":\"查依赖\",\"prompt\":\"列出 Cargo.toml 里的依赖。\"}"}),
        ),
        entry(
            json!({"id": "e9", "kind": "notice", "what": "job", "job": "j1", "job_kind": "agent",
            "title": "查依赖", "mark": "done", "reason": "done", "report": "一共 12 个依赖。"}),
        ),
        entry(json!({"id": "b11.0", "kind": "reply", "text": "查完了。", "persona": null})),
    ];
    let markdown = text(&entries);
    assert!(
        markdown.contains(
            "<details>\n<summary>交代</summary>\n\n列出 Cargo.toml 里的依赖。\n\n</details>"
        ),
        "{markdown}"
    );
    assert!(
        markdown.contains("## Subagent: 查依赖\n\n一共 12 个依赖。\n\n## Assistant\n\n查完了。"),
        "{markdown}"
    );
}

#[test]
fn attachments_and_pictures_are_placeholders() {
    let entries = vec![entry(
        json!({"id": "m2", "kind": "user", "text": "看图", "attachments": [
            {"kind": "image", "name": "shot.png", "media_type": "image/png",
             "blob": format!("sha256:{}", "a".repeat(64)), "width": 800, "height": 600},
            {"kind": "file", "media_type": "application/pdf", "blob": format!("sha256:{}", "b".repeat(64))},
        ]}),
    )];
    let markdown = text(&entries);
    assert!(
        markdown.contains("看图\n\n- [图片：shot.png, 800×600]\n- [文件]"),
        "{markdown}"
    );
    assert!(!markdown.contains("sha256"), "不内联图：{markdown}");
}

#[test]
fn the_home_folder_is_written_as_a_tilde_only_where_it_is_the_whole_folder() {
    let entries = vec![entry(json!({"id": "m2", "kind": "user",
        "text": "/home/alice/a 和 /home/alice 和 /home/alicex 和 /x/home/alice/b"}))];
    let markdown = text(&entries);
    assert!(
        markdown.contains("~/a 和 ~ 和 /home/alicex 和 /x/home/alice/b"),
        "{markdown}"
    );
}

#[test]
fn configured_secrets_are_redacted_and_counted() {
    let entries = vec![
        entry(
            json!({"id": "m2", "kind": "user", "text": "key 是 sk-live-123456789，再说一遍 sk-live-123456789"}),
        ),
        entry(json!({"id": "b4.0", "kind": "reply", "persona": null,
            "text": "收到 sk-live-123456789-extra，短的 abc 不换"})),
    ];
    let secrets = [
        ("OPENAI_KEY", "sk-live-123456789"),
        ("LONGER", "sk-live-123456789-extra"),
        ("SHORT", "abc"),
    ];
    let rendered = zh(&entries, &Sources::default(), &secrets);
    let markdown = &rendered.text;
    assert!(
        markdown.contains("key 是 [redacted: OPENAI_KEY]，再说一遍 [redacted: OPENAI_KEY]"),
        "{markdown}"
    );
    assert!(
        markdown.contains("收到 [redacted: LONGER]，短的 abc 不换"),
        "同一处起头的取长的：{markdown}"
    );
    assert_eq!(
        rendered.redacted,
        [("LONGER".to_string(), 1), ("OPENAI_KEY".to_string(), 2)].into(),
        "太短的不换、不报"
    );
}

#[test]
fn notices_and_errors_are_one_line_each() {
    let entries = vec![
        entry(
            json!({"id": "e3", "kind": "notice", "what": "workspace", "cwd": "/home/alice/proj"}),
        ),
        entry(
            json!({"id": "e5", "kind": "end", "reason": "error", "took_ms": 10,
            "explain": "认证失败：\nkey 不对"}),
        ),
        entry(json!({"id": "e6", "kind": "notice", "what": "cleared"})),
    ];
    let markdown = text(&entries);
    assert!(
        markdown.contains(
            "> 工作目录换成了 ~/proj\n\n> 这一轮出错了：认证失败： key 不对\n\n> 上下文清空了"
        ),
        "{markdown}"
    );
    let compaction = EntryId::compaction(Seq::new(4).expect("非零"));
    assert_eq!(compaction.as_str(), "c4", "摘要照压缩那一条的编号记");
}
