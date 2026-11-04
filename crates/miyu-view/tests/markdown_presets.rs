//! `agent`、`chat` 两个预设和盖过预设的开关（施工 SE-1 中下中，`miyu_view::markdown`）：各一整段逐字对；工具写真名，`agent`
//! 写参数原文、结果截到头尾约 4000 字、交代在参数里、开头一段英文事实；`chat` 一行写完、出错的带上错误、只写回报、开头写 token
//! 估算（同回应的 `tokens`）；写了的开关（思考、工具写多少、范围）盖过预设。

use serde_json::json;

use miyu_kernel::id::{CallId, Seq};
use miyu_kernel::time::UtcOffset;
use miyu_view::Entry;
use miyu_view::markdown::{AGENT_NOTE, Document, Options, Range, Rendered, Sources, Tools, render};

use crate::markdown::{about, entry};
use crate::support::*;

/// 一轮：问、想、读文件、编译出错、派子代理、回报、答、收尾。
fn session() -> Vec<Entry> {
    vec![
        entry(json!({"id": "m2", "kind": "user", "text": "帮我看看 main.rs", "turn": 3})),
        entry(
            json!({"id": "b4.0", "kind": "thought", "group": "g4.0", "text": "先读一下。", "took_ms": 2000, "turn": 3}),
        ),
        entry(
            json!({"id": "b4.1", "kind": "tool", "group": "g4.0", "call": "call_5_1", "name": "read", "turn": 3,
            "title": {"name": "读取", "object": "/home/alice/src/main.rs", "said": "读了 3 行"},
            "state": "ok", "args": "{\"file_path\":\"/home/alice/src/main.rs\"}",
            "output": "fn main() {\n    println!(\"hi\");\n}"}),
        ),
        entry(
            json!({"id": "b4.2", "kind": "tool", "group": "g4.0", "call": "call_5_2", "name": "shell", "turn": 3,
            "title": {"name": "执行命令", "object": "cargo build"}, "state": "error",
            "args": "{\"command\":\"cargo build\",\"description\":\"cargo build\"}",
            "output": "error[E0425]: cannot find value `x`\n  --> src/main.rs:2:5"}),
        ),
        entry(
            json!({"id": "b4.3", "kind": "tool", "group": "g4.0", "call": "call_5_3", "name": "subagent", "turn": 3,
            "title": {"name": "子代理", "object": "查依赖"}, "state": "ok", "job": "j1",
            "args": "{\"description\":\"查依赖\",\"prompt\":\"列出依赖。\"}", "output": "started j1"}),
        ),
        entry(
            json!({"id": "e9", "kind": "notice", "what": "job", "job": "j1", "job_kind": "agent", "turn": 3,
            "title": "查依赖", "mark": "done", "reason": "done", "report": "一共 12 个依赖。"}),
        ),
        entry(
            json!({"id": "b10.0", "kind": "reply", "text": "就是打一句 `hi`。", "persona": null, "turn": 3}),
        ),
        entry(
            json!({"id": "e11", "kind": "end", "reason": "completed", "took_ms": 3000, "turn": 3,
            "endpoint": "deepseek", "model": "deepseek-chat"}),
        ),
    ]
}

/// 照 `options` 渲染成中文，东九区。
pub(crate) fn rendered(entries: &[Entry], sources: &Sources, options: Options) -> Rendered {
    let texts = texts("zh");
    render(&Document {
        entries,
        about: &about(),
        sources,
        texts: &texts,
        options,
        offset: UtcOffset::from_minutes(540).expect("在范围里"),
        secrets: &[],
    })
}

fn markdown(options: Options) -> String {
    rendered(&session(), &Sources::default(), options).text
}

#[test]
fn the_agent_preset_reads_like_a_record_for_another_agent() {
    let options = Options {
        range: Range::All,
        ..Options::AGENT
    };
    let expected = format!(
        "\
# Session transcript

{AGENT_NOTE}

- Working directory: ~/proj
- Time: 2026-11-04 10:00 – 10:32 (UTC+09:00)
- Model: deepseek/deepseek-chat

## User

帮我看看 main.rs

## Assistant

### Tool: read

```json
{{\"file_path\":\"~/src/main.rs\"}}
```

```text
fn main() {{
    println!(\"hi\");
}}
```

### Tool: shell (error)

```json
{{\"command\":\"cargo build\",\"description\":\"cargo build\"}}
```

```text
error[E0425]: cannot find value `x`
  --> src/main.rs:2:5
```

### Tool: subagent

```json
{{\"description\":\"查依赖\",\"prompt\":\"列出依赖。\"}}
```

```text
started j1
```

## Subagent: 查依赖

一共 12 个依赖。

## Assistant

就是打一句 `hi`。
"
    );
    let got = markdown(options);
    assert_eq!(got, expected, "{got}");
    assert!(
        !AGENT_NOTE.contains("Please") && !AGENT_NOTE.contains("must"),
        "只写事实，不写指令"
    );
}

#[test]
fn the_chat_preset_writes_each_tool_on_one_line_and_estimates_tokens() {
    let got = markdown(Options::CHAT);
    let tokens = got.len().div_ceil(4);
    let expected = format!(
        "\
# 看看 main.rs

- 时间：2026-11-04 10:00 – 10:32 (UTC+09:00)
- token 估算：约 {tokens}

## User

帮我看看 main.rs

## Assistant

- Tool: `read` `~/src/main.rs`
- Tool: `shell` `cargo build` · error: error[E0425]: cannot find value `x` --> src/main.rs:2:5
- Tool: `subagent` `查依赖`

## Subagent: 查依赖

一共 12 个依赖。

## Assistant

就是打一句 `hi`。
"
    );
    assert_eq!(got, expected, "{got}");
    assert_eq!(
        miyu_kernel::estimate::text(&got),
        u64::try_from(tokens).expect("放得下"),
        "开头的估算同整份的估算，数本身也算在里面"
    );
    for pad in 1..=8 {
        let mut entries = session();
        entries[0] = entry(json!({"id": "m2", "kind": "user", "turn": 3,
            "text": format!("帮我看看 main.rs{}", "!".repeat(pad))}));
        let got = rendered(&entries, &Sources::default(), Options::CHAT).text;
        let wanted = format!("- token 估算：约 {}\n", miyu_kernel::estimate::text(&got));
        assert!(got.contains(&wanted), "长短不同照样对得上（{pad}）：{got}");
    }
}

#[test]
fn agent_results_keep_about_4000_characters_head_and_tail_unless_full() {
    let call = CallId::new(Seq::new(5).expect("非零"), 1).expect("从 1 数起");
    let lines: Vec<String> = (1..=2000).map(|n| format!("line {n:04}")).collect();
    let whole = lines.join("\n");
    let mut sources = Sources::default();
    sources.outputs.insert(call, whole.clone());
    let entries = vec![entry(
        json!({"id": "b4.1", "kind": "tool", "group": "g4.0", "call": "call_5_1", "name": "shell",
        "title": {"name": "执行命令", "object": "seq"}, "state": "ok", "args": "{}",
        "output": "line 0001", "output_cut": true}),
    )];
    let seen = rendered(&entries, &sources, Options::AGENT).text;
    assert!(seen.contains("```text\nline 0001\n"), "头照留：{seen}");
    assert!(seen.contains("line 2000\n```"), "尾照留：{seen}");
    assert!(!seen.contains("line 1000\n"), "中间省掉：{seen}");
    let marker = seen
        .lines()
        .find(|line| line.starts_with("[… "))
        .expect("中间一行写省了几个字");
    assert!(marker.ends_with(" characters omitted …]"), "{marker}");
    let body = seen.split("```text\n").nth(1).expect("有结果");
    let kept = body.chars().count() - marker.chars().count();
    assert!((3800..=4200).contains(&kept), "头尾共约 4000 字：{kept}");
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
        full.contains(&format!("```text\n{whole}\n```")),
        "full 不截"
    );
}

#[test]
fn switches_override_the_preset() {
    let thinking = markdown(Options {
        thinking: true,
        ..Options::CHAT
    });
    assert!(
        thinking.contains("<summary>思考了 2s</summary>\n\n先读一下。"),
        "chat 写了 thinking 带思考：{thinking}"
    );
    let plain = markdown(Options::CHAT);
    assert!(!plain.contains("先读一下"), "chat 默认不带：{plain}");
    let agent = markdown(Options::AGENT);
    assert!(!agent.contains("先读一下"), "agent 默认不带：{agent}");
    let seen = markdown(Options {
        tools: Tools::Seen,
        ..Options::CHAT
    });
    assert!(
        seen.contains("### Tool: read\n\n```json\n{\"file_path\":\"~/src/main.rs\"}\n```"),
        "chat 写了 tools: seen 照真名、参数原文：{seen}"
    );
    let line = markdown(Options {
        tools: Tools::Line,
        ..Options::READ
    });
    assert!(
        line.contains(
            "- Tool: 读取 `~/src/main.rs` · 读了 3 行\n- Tool: 执行命令 `cargo build` · 出错: error[E0425]"
        ),
        "read 写了 tools: line 照显示名一行：{line}"
    );
    let line = markdown(Options {
        tools: Tools::Line,
        ..Options::AGENT
    });
    assert!(
        line.contains("- Tool: `read` `~/src/main.rs`\n- Tool: `shell`"),
        "agent 写了 tools: line 照真名一行：{line}"
    );
    let last = markdown(Options {
        range: Range::Turns(1),
        ..Options::READ
    });
    assert_eq!(
        last,
        markdown(Options::READ),
        "只有一轮的，最近一轮就是全部"
    );
}
