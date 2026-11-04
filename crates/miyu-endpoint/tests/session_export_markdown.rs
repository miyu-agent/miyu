//! 会话导出成 Markdown（施工 SE-1 中下上，`docs/blueprint/protocol.md`「`session.export`」）：真核心跑几轮（调工具、思考、
//! 手动压缩）以后导到文件、放进回应，两份一字不差；配了的密钥换成占位、报几处；照参数换语言；放进回应超了上限的拒；下一步
//! 才有的写法拒 `bad_params` 带 `data.later`，写文件的规矩同原始包。

use std::path::Path;

use serde_json::{Value, json};

use miyu_session::testkit::{Play, Script};
use miyu_tool::Catalog;

use crate::support::*;

/// 密钥的值：会话里人说了一次、读的文件里有一次。
const SECRET: &str = "sk-test-abcdef123456";

fn tools() -> Catalog {
    let resources = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources");
    Catalog::new(miyu_basesystem::tools(&resources).expect("出厂的资源读得出来")).expect("合写法")
}

async fn markdown(client: &mut Client, id: &str, params: Value) -> Value {
    let mut params = params;
    params["format"] = json!("markdown");
    client.call(id, "session.export", params).await
}

/// 跑三轮：第一句说七万个字（压得掉），第二轮读一个文件、想一想再答，手动压一次，第三轮说再见。交回客户端和会话。
async fn three_turns(home: &Home) -> (Client, String) {
    std::fs::write(home.work.join("a.txt"), format!("token={SECRET}\n")).expect("写得进工作区");
    let script = Script::new([
        Play::Says("好。"),
        Play::calls(&[("read", r#"{"file_path":"a.txt"}"#)]),
        Play::Thinks {
            thinking: "文件里有个 token。",
            text: "读完了。",
        },
        Play::Says("<analysis>a</analysis>\n<summary>\nThe user read a file.\n</summary>"),
        Play::Says("再见。"),
    ])
    .window(1_000_000);
    let mut client = Client::connect(home.core_with_tools(&script, tools(), TOKEN));
    client.hello().await;
    let set = client
        .call(
            "k1",
            "secret.set",
            json!({"name": "demo_key", "value": SECRET}),
        )
        .await;
    assert!(set.get("error").is_none(), "{set}");
    let session = client.create("c1", &home.work.to_string_lossy()).await;
    client.say("s1", &session, &"x".repeat(70_000)).await;
    home.until_turns(&session, 1).await;
    client
        .say("s2", &session, &format!("读一下 a.txt，key 是 {SECRET}"))
        .await;
    home.until_turns(&session, 2).await;
    client
        .call("c2", "session.compact", json!({"session": session}))
        .await;
    home.until_turns(&session, 3).await;
    client.say("s3", &session, "再见").await;
    home.until_turns(&session, 4).await;
    (client, session)
}

#[tokio::test]
async fn a_session_reads_the_same_in_a_file_and_inline_with_secrets_redacted() {
    let home = Home::new();
    let (mut client, session) = three_turns(&home).await;
    let out = home.work.join("talk.md");
    let written = markdown(
        &mut client,
        "e1",
        json!({"session": session, "to": {"path": out}}),
    )
    .await;
    let result = &written["result"];
    let text = std::fs::read_to_string(&out).expect("写出来了");
    assert_eq!(result["path"], json!(out), "{written}");
    assert_eq!(result["bytes"], json!(text.len()), "{written}");
    assert_eq!(
        result["tokens"],
        json!(text.len().div_ceil(4)),
        "照字节数除以 4 估、往上取整"
    );
    let last = home.log(&session).last().expect("有日志").seq.get();
    assert_eq!(result["upto"], json!(last), "导到落了盘的最后一条");
    assert_eq!(
        result["redacted"],
        json!({"demo_key": 2}),
        "人说的、文件里读到的各一处：{written}"
    );
    assert!(!text.contains(SECRET), "{text}");
    for wanted in [
        "## User\n\n读一下 a.txt，key 是 [redacted: demo_key]",
        "## Assistant",
        "### Tool: 读取 `a.txt`",
        "token=[redacted: demo_key]",
        "<summary>思考",
        "文件里有个 token。",
        "读完了。",
        "> 上下文压缩了",
        "<details>\n<summary>摘要</summary>\n\nThe user read a file.\n\n</details>",
        "## User\n\n再见\n\n## Assistant\n\n再见。",
        "- token 合计：",
    ] {
        assert!(text.contains(wanted), "少了 {wanted:?}：\n{text}");
    }
    let compacted = text.find("> 上下文压缩了").expect("有");
    assert!(
        text.find("读完了。").expect("有") < compacted
            && compacted < text.find("再见。").expect("有"),
        "压缩照时间线放：\n{text}"
    );
    let inline = markdown(
        &mut client,
        "e2",
        json!({"session": session, "to": "inline", "for": "read", "range": "all"}),
    )
    .await;
    assert_eq!(
        inline["result"]["text"],
        json!(text),
        "放进回应的和写进文件的一字不差"
    );
    assert!(inline["result"].get("path").is_none(), "{inline}");
    let english = markdown(
        &mut client,
        "e3",
        json!({"session": session, "to": "inline", "locale": "en-US"}),
    )
    .await;
    let english = english["result"]["text"].as_str().expect("有字");
    assert!(
        english.contains("> Context compacted") && english.contains("### Tool: Read"),
        "照参数换语言，角色还是英文：\n{english}"
    );
}

#[tokio::test]
async fn a_big_session_is_refused_inline_and_still_goes_to_a_file() {
    let home = Home::new();
    let script = Script::new([Play::Says("好。")]).window(1_000_000);
    let mut client = Client::connect(home.core(&script));
    client.hello().await;
    let session = client.create("c1", "~").await;
    client.say("s1", &session, &"y".repeat(600 * 1024)).await;
    home.until_turns(&session, 1).await;
    let inline = markdown(
        &mut client,
        "e1",
        json!({"session": session, "to": "inline"}),
    )
    .await;
    assert_eq!(reason(&inline), Some("export_too_big"), "{inline}");
    assert_eq!(
        inline["error"]["message"],
        json!("导出超过 512 KiB，请存成文件。")
    );
    let out = home.work.join("big.md");
    let written = markdown(
        &mut client,
        "e2",
        json!({"session": session, "to": {"path": out}}),
    )
    .await;
    assert!(
        written["result"]["bytes"]
            .as_u64()
            .is_some_and(|n| n > 512 * 1024),
        "{written}"
    );
}

#[tokio::test]
async fn later_options_and_bad_places_are_refused() {
    let home = Home::new();
    let script = Script::new([Play::Says("好。")]);
    let mut client = Client::connect(home.core(&script));
    client.hello().await;
    let session = client.create("c1", "~").await;
    client.say("s1", &session, "你好").await;
    home.until_turns(&session, 1).await;
    let later = [
        (json!({"for": "agent"}), "for"),
        (json!({"for": "chat"}), "for"),
        (json!({"range": "context"}), "range"),
        (json!({"range": {"turns": 3}}), "range"),
        (json!({"thinking": false}), "thinking"),
        (json!({"tools": "line"}), "tools"),
        (json!({"children": "append"}), "children"),
        (json!({"assets": "files"}), "assets"),
    ];
    for (n, (extra, param)) in later.into_iter().enumerate() {
        let mut params = json!({"session": session, "to": "inline"});
        for (key, value) in extra.as_object().expect("是对象") {
            params[key] = value.clone();
        }
        let reply = markdown(&mut client, &format!("l{n}"), params).await;
        assert_eq!(reason(&reply), Some("bad_params"), "{reply}");
        assert_eq!(reply["error"]["data"]["later"], json!(param), "{reply}");
    }
    let taken = home.work.join("taken.md");
    std::fs::write(&taken, "人家的").expect("写得进");
    let bad = [
        (json!({"for": "nobody", "to": "inline"}), "bad_params"),
        (json!({"range": {"turns": 0}, "to": "inline"}), "bad_params"),
        (json!({"to": "elsewhere"}), "bad_params"),
        (json!({"to": {"path": "x.md"}}), "bad_params"),
        (json!({"to": {"path": taken}}), "export_exists"),
        (
            json!({"to": {"path": home.root.path().join("in.md")}}),
            "export_in_data_root",
        ),
        (
            json!({"to": {"path": home.work.join("no-such-dir").join("x.md")}}),
            "export_unwritable",
        ),
    ];
    for (n, (params, wanted)) in bad.into_iter().enumerate() {
        let mut params = params;
        params["session"] = json!(session);
        let reply = markdown(&mut client, &format!("b{n}"), params).await;
        assert_eq!(reason(&reply), Some(wanted), "{reply}");
        assert!(reply["error"]["data"].get("later").is_none(), "{reply}");
    }
    assert_eq!(std::fs::read_to_string(&taken).expect("在"), "人家的");
    let missing = markdown(
        &mut client,
        "n1",
        json!({"session": "0192f3a0-0000-7000-8000-000000000099", "to": "inline"}),
    )
    .await;
    assert_eq!(reason(&missing), Some("session_not_found"), "{missing}");
    for (n, params) in [
        json!({"session": session, "format": "bundle", "to": "inline"}),
        json!({"session": session, "format": "bundle", "for": "read", "to": {"path": home.work.join("b.zip")}}),
    ]
    .into_iter()
    .enumerate()
    {
        let reply = client.call(&format!("z{n}"), "session.export", params).await;
        assert_eq!(reason(&reply), Some("bad_params"), "原始包不收 Markdown 的参数：{reply}");
    }
}
