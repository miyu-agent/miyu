//! 会话导出成 Markdown 的三个预设和范围（施工 SE-1 中下中，`docs/blueprint/protocol.md`「`session.export`」）：真核心跑四轮
//! （七万字、读文件加思考、手动压缩、再一轮），`read`、`agent`、`chat` 各导一份；`agent` 默认从检查点起（压掉的是那七万字，
//! 后面那一截照有效历史留着）、摘要放最前；`read` 的 `context` 同一段、压缩那一条照时间线；`{"turns": N}` 写省掉了几轮；
//! 开关盖过预设；原始包里的 `transcript.md` 是 `read` 预设那一份、换过密钥，日志原样，导入照常。

use std::fs::File;
use std::io::Read;
use std::path::Path;

use serde_json::{Value, json};
use zip::ZipArchive;

use miyu_session::testkit::{Play, Script};
use miyu_store::bundle::Manifest;
use miyu_view::markdown::AGENT_NOTE;

use crate::session_export_markdown::{SECRET, markdown, three_turns};
use crate::support::*;

/// 放进回应导一份：交回字和回应的 `tokens`。
async fn inline(client: &mut Client, id: &str, session: &str, params: Value) -> (String, u64) {
    let mut params = params;
    params["session"] = json!(session);
    params["to"] = json!("inline");
    let reply = markdown(client, id, params).await;
    let text = reply["result"]["text"]
        .as_str()
        .unwrap_or_else(|| panic!("{reply}"))
        .to_string();
    let tokens = reply["result"]["tokens"].as_u64().expect("有估算");
    (text, tokens)
}

fn has(text: &str, wanted: &[&str]) {
    for one in wanted {
        assert!(text.contains(one), "少了 {one:?}：\n{text}");
    }
}

fn lacks(text: &str, unwanted: &[&str]) {
    for one in unwanted {
        assert!(!text.contains(one), "不该有 {one:?}：\n{text}");
    }
}

#[tokio::test]
async fn three_presets_from_a_real_core() {
    let home = Home::new();
    let (mut client, session) = three_turns(&home).await;
    let long = "x".repeat(1000);

    let (agent, _) = inline(&mut client, "a1", &session, json!({"for": "agent"})).await;
    assert!(
        agent.starts_with(&format!(
            "# Session transcript\n\n{AGENT_NOTE}\n\n- Working directory: "
        )),
        "{agent}"
    );
    has(
        &agent,
        &[
            "- Earlier parts of the session were compacted; the summary below stands in for them.\n\n## Summary\n\nThe user read a file.\n\n## Assistant\n\n好。\n\n## User\n\n读一下 a.txt",
            "### Tool: read\n\n```json\n{\"file_path\":\"a.txt\"}\n```\n\n```text\n",
            "读完了。\n\n## User\n\n再见\n\n## Assistant\n\n再见。\n",
        ],
    );
    lacks(
        &agent,
        &[&long, "<details>", "上下文压缩了", "文件里有个 token。"],
    );

    let (all, _) = inline(
        &mut client,
        "a2",
        &session,
        json!({"for": "agent", "range": "all"}),
    )
    .await;
    has(
        &all,
        &[
            "### Tool: read\n\n```json\n{\"file_path\":\"a.txt\"}\n```\n\n```text\n",
            "token=[redacted: demo_key]",
            "读完了。",
            &long,
        ],
    );
    lacks(&all, &["文件里有个 token。", "### Tool: 读取", SECRET]);

    let (chat, tokens) = inline(&mut client, "c1", &session, json!({"for": "chat"})).await;
    has(
        &chat,
        &[
            &format!("- token 估算：约 {tokens}\n"),
            "## Assistant\n\n- Tool: `read` `a.txt`\n\n读完了。",
            "> 上下文压缩了",
            &long,
        ],
    );
    lacks(&chat, &["文件里有个 token。", "```json", "token 合计"]);

    let (read, _) = inline(&mut client, "r1", &session, json!({"for": "read"})).await;
    has(
        &read,
        &[
            "### Tool: 读取 `a.txt`",
            "文件里有个 token。",
            "- token 合计：",
        ],
    );

    let (thinking, _) = inline(
        &mut client,
        "o1",
        &session,
        json!({"for": "agent", "range": "all", "thinking": true, "tools": "line"}),
    )
    .await;
    has(&thinking, &["文件里有个 token。", "- Tool: `read` `a.txt`"]);
    lacks(&thinking, &["```json"]);
}

#[tokio::test]
async fn context_and_last_turns_say_what_is_left_out() {
    let home = Home::new();
    let (mut client, session) = three_turns(&home).await;
    let (context, _) = inline(&mut client, "x1", &session, json!({"range": "context"})).await;
    has(
        &context,
        &[
            "> 上下文压缩了",
            "## User\n\n读一下 a.txt",
            "## User\n\n再见",
        ],
    );
    lacks(&context, &["x".repeat(1000).as_str(), "省掉"]);
    let (last, _) = inline(
        &mut client,
        "x2",
        &session,
        json!({"for": "chat", "range": {"turns": 1}}),
    )
    .await;
    has(&last, &["\n\n> 前面省掉了 3 轮\n\n## User\n\n再见"]);
    lacks(&last, &["上下文压缩了", "读完了。"]);
    let (agent, _) = inline(
        &mut client,
        "x3",
        &session,
        json!({"for": "agent", "range": {"turns": 2}, "locale": "en"}),
    )
    .await;
    has(
        &agent,
        &["- Only the latest turns are included; 2 earlier turns are left out."],
    );
}

fn entry(path: &Path, name: &str) -> Vec<u8> {
    let mut archive = ZipArchive::new(File::open(path).expect("包在")).expect("是 ZIP");
    let mut file = archive
        .by_name(name)
        .unwrap_or_else(|_| panic!("包里有 {name}"));
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).expect("读得出");
    bytes
}

#[tokio::test]
async fn the_bundle_carries_the_read_transcript_with_secrets_redacted_and_still_imports() {
    let home = Home::new();
    let (mut client, session) = three_turns(&home).await;
    let out = home.work.join("talk.zip");
    let reply = client
        .call(
            "b1",
            "session.export",
            json!({"session": session, "format": "bundle", "to": {"path": out}}),
        )
        .await;
    assert!(reply.get("error").is_none(), "{reply}");
    let transcript = String::from_utf8(entry(&out, "transcript.md")).expect("是字");
    let (read, _) = inline(&mut client, "r1", &session, json!({})).await;
    assert_eq!(transcript, read, "就是 read 预设那一份");
    assert!(
        transcript.contains("[redacted: demo_key]") && !transcript.contains(SECRET),
        "{transcript}"
    );
    let manifest: Manifest =
        serde_json::from_slice(&entry(&out, "manifest.json")).expect("清单读得回");
    let listed = manifest.transcript.expect("清单记了它");
    assert_eq!(listed.bytes, transcript.len() as u64);
    let segment = &manifest.sessions[0].segments[0];
    let log = String::from_utf8(entry(&out, &format!("sessions/{session}/{}", segment.name)))
        .expect("是字");
    assert!(log.contains(SECRET), "日志原样不改");

    let other = Home::new();
    let script = Script::new([Play::Says("好。")]);
    let mut there = Client::connect(other.core(&script));
    there.hello().await;
    let imported = there
        .call("i1", "session.import", json!({"path": out}))
        .await;
    assert_eq!(
        imported["result"]["sessions"],
        json!([session]),
        "{imported}"
    );
}
