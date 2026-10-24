//! 场所显示不了图片（施工 X-25，`docs/blueprint/venues.md`「`venue.session`」的 `images`）：`session.create` 写了 `images: false`
//! 的会话里，清单写了 `needs = ["images"]` 的包整包当没开：工具不进工具面、回合开始不问它、它放的 system 那一段不拼，也不算进
//! 「装了没开」；会话退下再载入、预设的文件改了、换快照以后照旧扣着。别的包、显示得了图片的会话一切照旧，`session.created` 不写这一格。

use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};

use miyu_kernel::event::Body;
use miyu_session::testkit::{Play, Script};
use miyu_tool::Catalog;

use crate::support::extensions::*;
use crate::support::*;

/// 包 `id` 提供的一件工具。
fn tools(id: &str) -> Value {
    json!({"tools": [{"name": format!("{id}_tool"), "description": "A test tool.",
                      "input_schema": {"type": "object"}, "access": "read", "venues": ["local"]}]})
}

/// 等到 `path` 里记下的有 `n` 行。
async fn lines(path: &std::path::Path, n: usize) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    while read(path).lines().count() < n {
        assert!(tokio::time::Instant::now() < deadline, "等不到 {n} 行");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

/// 会话 `session` 现在的快照（最后一次换上的，没换过的是造会话时的）里，工具面上的工具名、system 尾部是哪几个包放的、没开的
/// 软件。
fn face(home: &Home, session: &str) -> (Vec<String>, Vec<String>, Vec<String>) {
    let policy = home
        .log(session)
        .iter()
        .filter_map(|event| match &event.body {
            Body::SessionCreated(created) => Some(created.policy.clone()),
            Body::PolicyChanged(changed) => changed.policy.clone(),
            _ => None,
        })
        .next_back()
        .expect("有快照");
    let bytes = miyu_store::blob::Blobs::new(home.root.blobs(&alice()))
        .get(&policy)
        .expect("快照在 blob 里");
    let snapshot = miyu_policy::Snapshot::from_bytes(&bytes).expect("读得懂");
    let tools = snapshot
        .tools
        .iter()
        .map(|tool| tool.name.clone())
        .collect();
    let appended = snapshot.appended.keys().cloned().collect();
    let off = snapshot.preset.map(|pin| pin.off).unwrap_or_default();
    (tools, appended, off)
}

/// 会话 `session` 的日志里，回合开始加进来的字是哪几个包交的。
fn injected_by(home: &Home, session: &str) -> Vec<String> {
    home.log(session)
        .iter()
        .filter_map(|event| match &event.body {
            Body::ContextInjected(fact) if fact.kind.as_str() == "meme" => {
                Some(format!("{:?}", event.by))
            }
            _ => None,
        })
        .collect()
}

#[tokio::test]
async fn a_session_that_cannot_show_images_leaves_out_the_packages_that_need_them() {
    let home = Home::new();
    // 编号、要不要图。
    let packages = [("memes", true), ("plain", false)];
    let programs: Vec<Program> = packages.iter().map(|_| Program::new()).collect();
    let capabilities = ["tools", "context.inject", "context.system"];
    let mut records = Vec::new();
    for ((id, needs), program) in packages.iter().zip(&programs) {
        let (path, step) = record(&home, id);
        let append = format!(r#"ask:system.append:{{"content":"<{id}>X</{id}>"}}"#);
        install_asking(
            &home,
            id,
            &program.name(),
            "always",
            &steps(&[
                &step,
                "hello",
                &format!("ask:provide:{}", tools(id)),
                &append,
                "inject:hint",
                "serve",
            ]),
            &capabilities,
        );
        if *needs {
            let manifest = format!("system/packages/{id}/package.toml");
            let text = read(&home.root.path().join(&manifest));
            home.write(
                &manifest,
                &text.replacen("[package]\n", "[package]\nneeds = [\"images\"]\n", 1),
            );
        }
        approve(&home, id, &capabilities);
        records.push(path);
    }
    let script = Script::new([Play::Says("一。"), Play::Says("二。"), Play::Says("三。")])
        // 两个会话各起一次标题：没排上的，假模型不回，会话一直有事、退不下。
        .titles([Play::Says("测试"), Play::Says("测试")]);
    let core = Arc::new(
        home.core_full(&script, Catalog::default(), None, TOKEN)
            .with_extension_timing(quick())
            .with_session_idle(Duration::from_millis(100)),
    );
    core.start_extensions();
    for path in &records {
        lines(path, 4).await;
    }

    home.write(
        "home/alice/presets/p.toml",
        "[preset]\nname = { en = \"P\" }\n",
    );
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let seeing = client.create("c1", "~").await;
    let blind = client
        .create_with("c2", json!({"cwd": "~", "images": false, "preset": "p"}))
        .await;

    let (tools, appended, off) = face(&home, &seeing);
    assert!(
        tools.contains(&"memes_tool".to_string()) && tools.contains(&"plain_tool".to_string()),
        "显示得了图片的照旧：{tools:?}"
    );
    assert_eq!(appended, ["memes", "plain"]);
    assert!(off.is_empty());
    let (tools, appended, off) = face(&home, &blind);
    assert!(
        !tools.contains(&"memes_tool".to_string()),
        "要图的包工具不进工具面：{tools:?}"
    );
    assert!(tools.contains(&"plain_tool".to_string()), "别的包照旧");
    assert_eq!(appended, ["plain"], "要图的包放的不拼");
    assert!(off.is_empty(), "扣下的不算装了没开：{off:?}");

    let created = |session: &str| {
        let log = home.log(session);
        let first = log.first().expect("有第一条");
        serde_json::to_value(&first.body).expect("写得成 JSON")
    };
    assert_eq!(created(&blind)["images"], json!(false), "显示不了的记下");
    assert!(
        created(&seeing).get("images").is_none(),
        "显示得了的不写，以前的字节不变"
    );

    client.say("s1", &seeing, "一").await;
    home.until_turns(&seeing, 1).await;
    client.say("s2", &blind, "二").await;
    home.until_turns(&blind, 1).await;
    assert_eq!(injected_by(&home, &seeing).len(), 2, "两个包都问");
    let asked = injected_by(&home, &blind);
    assert!(
        asked.len() == 1 && asked[0].contains("plain"),
        "回合开始不问要图的包：{asked:?}"
    );

    // 会话闲着退下，再载入；预设的文件改了（指纹变、开的东西不变）：载入以后换快照照旧扣着。
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    while core.loaded().await > 0 {
        assert!(tokio::time::Instant::now() < deadline, "等不到会话退下");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    home.write(
        "home/alice/presets/p.toml",
        "[preset]\nname = { en = \"P\" }\nunlisted = \"on\"\n",
    );
    client.say("s3", &blind, "三").await;
    home.until_turns(&blind, 2).await;
    let swapped = home.log(&blind).iter().any(
        |event| matches!(&event.body, Body::PolicyChanged(changed) if changed.policy.is_some()),
    );
    assert!(swapped, "先断言真的换了快照");
    let (tools, appended, _) = face(&home, &blind);
    assert!(
        !tools.contains(&"memes_tool".to_string()) && tools.contains(&"plain_tool".to_string()),
        "换了快照照旧扣着：{tools:?}"
    );
    assert_eq!(appended, ["plain"]);
    core.stop_extensions().await;
}
