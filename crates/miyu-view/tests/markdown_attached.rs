//! 导出 Markdown 的第三截（施工 SE-1 中下下，`miyu_view::markdown`）：`agent` 的旁白、占位一律英文（连接是中文也一样，人说的、
//! 她回的照原文）；`assets: files` 的图片、附件写成指向旁边文件夹的相对链接，取不到的照占位；`children: append` 的子会话照
//! 同一套开关渲染、开头换成子代理的小标题，照任务编号接在后面，密钥、token 估算照整份算。

use serde_json::json;

use miyu_kernel::id::{ContentHash, JobId, Seq, SessionId};
use miyu_kernel::time::UtcOffset;
use miyu_view::markdown::{
    AGENT_NOTE, Child, Document, Draft, Files, Options, Range, Sources, assets, draft, finish,
};
use miyu_view::{Entry, EntryId};

use crate::markdown::{about, entry};
use crate::support::*;

/// 一个哈希：`sha256:` 接 64 个 `c`。
fn hash(c: char) -> String {
    format!("sha256:{}", c.to_string().repeat(64))
}

fn blob(c: char) -> ContentHash {
    serde_json::from_value(json!(hash(c))).expect("合写法")
}

/// 附件、她发的图、结果里的图，再是每一种旁白和收尾。
fn everything() -> Vec<Entry> {
    vec![
        entry(
            json!({"id": "m2", "kind": "user", "text": "看图", "attachments": [
                {"kind": "image", "name": "shot.png", "media_type": "image/png", "blob": hash('a'), "width": 800, "height": 600},
                {"kind": "file", "media_type": "application/pdf", "blob": hash('b')},
            ]}),
        ),
        entry(
            json!({"id": "b4.0", "kind": "thought", "group": "g4.0", "text": "先看看。", "took_ms": 2000}),
        ),
        entry(
            json!({"id": "b4.1", "kind": "tool", "group": "g4.0", "call": "call_4_1", "name": "read",
            "title": {"name": "读取", "object": "a.png"}, "state": "ok", "args": "{}", "output": "ok",
            "images": [{"blob": hash('d'), "media_type": "image/png", "width": 100, "height": 50}]}),
        ),
        entry(json!({"id": "s5.0", "kind": "media", "alt": "一只猫",
            "image": {"kind": "image", "media_type": "image/webp", "blob": hash('e'), "width": 64, "height": 64}})),
        entry(json!({"id": "c5", "kind": "notice", "what": "compaction", "state": "done"})),
        entry(
            json!({"id": "e7", "kind": "notice", "what": "compaction", "state": "failed", "explain": "超时了"}),
        ),
        entry(json!({"id": "e8", "kind": "notice", "what": "cleared"})),
        entry(
            json!({"id": "e9", "kind": "notice", "what": "workspace", "cwd": "/home/alice/proj"}),
        ),
        entry(
            json!({"id": "e10", "kind": "notice", "what": "model", "model": "deepseek-chat", "why": "manual"}),
        ),
        entry(
            json!({"id": "e11", "kind": "notice", "what": "job", "job": "j2", "job_kind": "command",
            "title": "cargo build", "mark": "failed", "reason": "exited", "command": "cargo build"}),
        ),
        entry(
            json!({"id": "e12", "kind": "notice", "what": "lines", "job": "j3", "title": "tail",
            "text": "a\nb", "lines": 2, "omitted": 0}),
        ),
        entry(json!({"id": "e13", "kind": "notice", "what": "recap", "text": "做了这些。"})),
        entry(
            json!({"id": "e14", "kind": "notice", "what": "job", "job": "j1", "job_kind": "agent",
            "title": "查依赖", "mark": "stopped", "reason": "stopped", "report": "半截。"}),
        ),
        entry(
            json!({"id": "e15", "kind": "end", "reason": "error", "took_ms": 1, "explain": "认证失败"}),
        ),
        entry(json!({"id": "e16", "kind": "end", "reason": "interrupted", "took_ms": 1})),
        entry(json!({"id": "e17", "kind": "end", "reason": "step_limit", "took_ms": 1})),
        entry(json!({"id": "e18", "kind": "end", "reason": "aborted", "took_ms": 1})),
    ]
}

fn sources() -> Sources {
    let mut sources = Sources::default();
    let compaction = EntryId::compaction(Seq::new(5).expect("非零"));
    sources
        .summaries
        .insert(compaction, "前面在看图。".to_string());
    sources
}

/// 照中文的字、东九区起一份草稿。
fn drafted(
    entries: &[Entry],
    sources: &Sources,
    options: Options,
    child: Option<&Child>,
    files: Option<&Files>,
) -> Draft {
    let texts = texts("zh");
    draft(&Document {
        entries,
        about: &about(),
        sources,
        texts: &texts,
        options,
        offset: UtcOffset::from_minutes(540).expect("在范围里"),
        secrets: &[],
        child,
        files,
    })
}

#[test]
fn the_agent_preset_writes_its_notes_in_english_whatever_the_locale() {
    let options = Options {
        range: Range::All,
        thinking: true,
        ..Options::AGENT
    };
    let text = finish(
        &[drafted(&everything(), &sources(), options, None, None)],
        &[],
        Some("/home/alice"),
    )
    .text;
    let expected = format!(
        "\
# Session transcript

{AGENT_NOTE}

- Working directory: ~/proj
- Time: 2026-11-04 10:00 – 10:32 (UTC+09:00)

## User

看图

- [Image: shot.png, 800×600]
- [File]

## Assistant

<details>
<summary>Thought for 2s</summary>

先看看。

</details>

### Tool: read

```json
{{}}
```

```text
ok
```

- [Image: 100×50]

[Image: 一只猫, 64×64]

> Context compacted

<details>
<summary>Summary</summary>

前面在看图。

</details>

> Compaction failed: 超时了

> Context cleared

> Working directory changed to ~/proj

> Model changed to deepseek-chat

> Background command failed: cargo build

> tail printed 2 lines

```text
a
b
```

> Recap

做了这些。

## Subagent: 查依赖

> The subagent stopped before finishing

半截。

> The turn ended with an error: 认证失败

> The turn was interrupted

> The turn hit the step limit

> The turn was cut off when Miyu stopped
"
    );
    assert_eq!(text, expected, "{text}");
    let read = finish(
        &[drafted(
            &everything(),
            &sources(),
            Options::READ,
            None,
            None,
        )],
        &[],
        None,
    )
    .text;
    for local in [
        "> 上下文压缩了",
        "[图片：shot.png, 800×600]",
        "> 这一轮被打断了",
    ] {
        assert!(read.contains(local), "read 照旧跟语言：{local}\n{read}");
    }
}

#[test]
fn assets_become_relative_links_into_the_folder_and_missing_ones_stay_placeholders() {
    let entries = everything();
    let wanted = assets(&entries);
    assert_eq!(
        wanted,
        [blob('a'), blob('b'), blob('d'), blob('e')].into(),
        "人附的、结果里的、她发的"
    );
    let mut files = Files::new("talk_files");
    files.have([blob('a'), blob('d'), blob('e')]);
    let text = drafted(&entries, &sources(), Options::READ, None, Some(&files)).text;
    assert!(
        text.contains(
            "看图\n\n- ![shot.png, 800×600](talk_files/shot.png)\n- [文件]\n\n## Assistant"
        ),
        "取不到的照占位：{text}"
    );
    assert!(
        text.contains("- ![100×50](talk_files/image-dddddddd.png)"),
        "{text}"
    );
    assert!(
        text.contains("\n\n![一只猫, 64×64](talk_files/image-eeeeeeee.webp)\n\n"),
        "{text}"
    );
    assert_eq!(
        files.named(),
        [
            (blob('a'), "shot.png".to_string()),
            (blob('d'), "image-dddddddd.png".to_string()),
            (blob('e'), "image-eeeeeeee.webp".to_string()),
        ]
        .into()
    );
    assert_eq!(files.missing(), [blob('b')].into());
}

/// 只有路径的附件（施工 AT-1）：没有 blob、类型，条目照样读得进；占位写它的路径，`assets: files` 不拷它、不算取不到的。
#[test]
fn a_path_only_attachment_is_written_as_its_path_and_never_copied() {
    let entries = vec![entry(
        json!({"id": "m2", "kind": "user", "text": "看看", "attachments": [
            {"kind": "file", "name": "disk.iso", "path": "/data/disk.iso", "size": 31457280, "path_only": true},
            {"kind": "file", "name": "a.pdf", "media_type": "application/pdf", "blob": hash('b')},
        ]}),
    )];
    assert_eq!(assets(&entries), [blob('b')].into(), "只有路径的没有可拷的");
    let mut files = Files::new("talk_files");
    files.have([blob('b')]);
    for files in [None, Some(&files)] {
        let text = drafted(&entries, &sources(), Options::READ, None, files).text;
        assert!(
            text.contains("看看\n\n- [文件：/data/disk.iso]\n- "),
            "{text}"
        );
    }
    assert!(files.missing().is_empty(), "不算取不到的");
    // 条目写出去：只有路径的那一件没有 `blob`、`media_type`，别的一格不多。
    let written = serde_json::to_value(&entries[0]).expect("写得成");
    assert_eq!(
        written["attachments"],
        json!([
            {"kind": "file", "name": "disk.iso", "path": "/data/disk.iso", "size": 31457280, "path_only": true},
            {"kind": "file", "name": "a.pdf", "media_type": "application/pdf", "blob": hash('b')},
        ])
    );
}

/// 主会话：派了两个子代理（先 j2 再 j1），一个撤掉了的派了 j3。
fn parent() -> (Vec<Entry>, Sources) {
    let tool = |id: &str, job: &str, hidden: bool| {
        entry(
            json!({"id": id, "kind": "tool", "group": "g4.0", "call": "call_4_1", "name": "subagent",
            "title": {"name": "子代理", "object": job}, "state": "ok", "job": job, "hidden": hidden,
            "args": "{}", "output": format!("started {job}")}),
        )
    };
    let entries = vec![
        entry(json!({"id": "m2", "kind": "user", "text": "key 是 sk-test-abcdef123456"})),
        tool("b4.1", "j2", false),
        tool("b4.2", "j1", false),
        tool("b4.3", "j3", true),
    ];
    let mut sources = Sources::default();
    for (job, n) in [("j1", 1), ("j2", 2), ("j3", 3)] {
        let session =
            SessionId::parse(&format!("0192f3a0-0000-7000-8000-00000000000{n}")).expect("合写法");
        let job = JobId::parse(job).expect("合写法");
        sources.jobs.insert(job, (session, format!("活 {n}")));
    }
    (entries, sources)
}

#[test]
fn appended_children_follow_in_job_order_under_their_own_heading() {
    let (entries, sources) = parent();
    let main = drafted(&entries, &sources, Options::CHAT, None, None);
    let jobs: Vec<String> = main.children.iter().map(|c| c.job.to_string()).collect();
    assert_eq!(jobs, ["j1", "j2"], "照任务编号，撤掉的那一步派的不带");
    let said = vec![entry(
        json!({"id": "m1", "kind": "user", "text": "子会话里也有 sk-test-abcdef123456"}),
    )];
    let mut drafts = vec![main.clone()];
    for child in &main.children {
        drafts.push(drafted(
            &said,
            &Sources::default(),
            Options::CHAT,
            Some(child),
            None,
        ));
    }
    let rendered = finish(&drafts, &[("demo", "sk-test-abcdef123456")], None);
    let text = &rendered.text;
    assert_eq!(
        rendered.redacted,
        [("demo".to_string(), 3)].into(),
        "{text}"
    );
    let estimate = text.len().div_ceil(4);
    assert!(
        text.contains(&format!("- token 估算：约 {estimate}\n")),
        "估算照整份：{text}"
    );
    assert_eq!(text.matches("token 估算").count(), 1, "{text}");
    assert!(
        text.ends_with(
            "\n\n---\n\n# Subagent j1: 活 1\n\n- 时间：2026-11-04 10:00 – 10:32 (UTC+09:00)\n\n## User\n\n子会话里也有 [redacted: demo]\n\n---\n\n# Subagent j2: 活 2\n\n- 时间：2026-11-04 10:00 – 10:32 (UTC+09:00)\n\n## User\n\n子会话里也有 [redacted: demo]\n"
        ),
        "{text}"
    );
    let child = &main.children[0];
    let read = drafted(&said, &Sources::default(), Options::READ, Some(child), None).text;
    assert!(
        read.starts_with(
            "---\n\n# Subagent j1: 活 1\n\n- 时间：2026-11-04 10:00 – 10:32 (UTC+09:00)\n- token 合计：1234\n\n## User"
        ),
        "{read}"
    );
    let agent = drafted(
        &said,
        &Sources::default(),
        Options::AGENT,
        Some(child),
        None,
    )
    .text;
    assert!(
        agent.starts_with(
            "---\n\n# Subagent j1: 活 1\n\n- Working directory: /home/alice/proj\n- Time: 2026-11-04 10:00 – 10:32 (UTC+09:00)\n\n## User"
        ),
        "不再写 AGENT_NOTE：{agent}"
    );
    assert!(!agent.contains(AGENT_NOTE), "{agent}");
}
