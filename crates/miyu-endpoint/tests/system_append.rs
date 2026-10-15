//! 扩展往 system 尾部放一段（施工 X-20，`system.append`）：声明了 `context.system` 的扩展放的，工具面上有它的工具的会话 system
//! 的最后带上，照包的编号排；改了下一轮换上，`null` 去掉以后和没放过的逐字节相同；没声明的回 `needs_approval`；工具不在这个
//! 会话工具面上的不带。

use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};

use miyu_session::testkit::{Play, Script};
use miyu_tool::Catalog;

use crate::support::extensions::*;
use crate::support::*;

/// 包 `id` 提供的一件工具，在 `venue` 这种场所。
fn tools(id: &str, venue: &str) -> Value {
    json!({"tools": [{"name": format!("{id}_tool"), "description": "A test tool.",
                      "input_schema": {"type": "object"}, "access": "read", "venues": [venue]}]})
}

/// 等到 `path` 里记下的有 `n` 行。
async fn lines(path: &std::path::Path, n: usize) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    while read(path).lines().count() < n {
        assert!(tokio::time::Instant::now() < deadline, "等不到 {n} 行");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

/// 放一段。
fn append(content: &str) -> String {
    format!(r#"ask:system.append:{{"content":{content}}}"#)
}

#[tokio::test]
async fn extensions_append_to_the_system_prompt_and_change_it_next_turn() {
    let home = Home::new();
    let gate = |n: usize| home.root.path().join(format!("go-{n}"));
    // 编号、声明不声明 `context.system`、工具在哪种场所、起来以后做的。
    let packages: [(&str, bool, &str, Vec<String>); 4] = [
        (
            "alpha",
            true,
            "local",
            vec![
                append(r#""<alpha>A1</alpha>""#),
                format!("pause:{}", gate(1).display()),
                append(r#""<alpha>A2</alpha>""#),
                format!("pause:{}", gate(2).display()),
                append("null"),
            ],
        ),
        (
            "bravo",
            true,
            "local",
            vec![append(r#""""#), append(r#""<bravo>B</bravo>""#)],
        ),
        (
            "charlie",
            false,
            "local",
            vec![append(r#""<charlie>C</charlie>""#)],
        ),
        ("golf", true, "group", vec![append(r#""<golf>G</golf>""#)]),
    ];
    let programs: Vec<Program> = packages.iter().map(|_| Program::new()).collect();
    let mut records = Vec::new();
    let mut approved = serde_json::Map::new();
    for ((id, declares, venue, after), program) in packages.iter().zip(&programs) {
        let (path, step) = record(&home, id);
        let capabilities: &[&str] = if *declares {
            &["tools", "context.system"]
        } else {
            &["tools"]
        };
        let provide = format!("ask:provide:{}", tools(id, venue));
        let mut all = vec![step, "hello".to_string(), provide];
        all.extend(after.iter().cloned());
        all.push("serve".to_string());
        let all: Vec<&str> = all.iter().map(String::as_str).collect();
        install_asking(
            &home,
            id,
            &program.name(),
            "always",
            &steps(&all),
            capabilities,
        );
        approved.insert(id.to_string(), json!(capabilities));
        records.push(path);
    }
    let system = home.root.system();
    std::fs::create_dir_all(&system).expect("建得了");
    std::fs::write(
        system.join("extensions.json"),
        format!(
            "{}\n",
            json!({"version": 1, "on": {}, "approved": approved})
        ),
    )
    .expect("写得进");
    let script = Script::new([Play::Says("一。"), Play::Says("二。"), Play::Says("三。")])
        .titles([Play::Says("测试")]);
    let core = Arc::new(
        home.core_full(&script, Catalog::default(), None, TOKEN)
            .with_extension_timing(quick()),
    );
    core.start_extensions();
    for path in &records {
        lines(path, 4).await;
    }
    lines(&records[1], 5).await;
    assert!(read(&records[1]).contains("bad_params"), "空的不收");
    let charlie = read(&records[2]);
    assert!(
        charlie.contains("needs_approval"),
        "没声明的不收：{charlie}"
    );

    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let session = client.create("c1", "~").await;
    let created =
        crate::support::venues::snapshot(&home, &alice(), &dir(&home, &alice(), &session));
    assert_eq!(
        created.appended.keys().collect::<Vec<_>>(),
        ["alpha", "bravo"],
        "造会话时就钉进去，第一轮不用再换一次"
    );
    client.say("s1", &session, "一").await;
    home.until_turns(&session, 1).await;
    std::fs::write(gate(1), "").expect("写得进");
    lines(&records[0], 5).await;
    client.say("s2", &session, "二").await;
    home.until_turns(&session, 2).await;
    std::fs::write(gate(2), "").expect("写得进");
    lines(&records[0], 6).await;
    client.say("s3", &session, "三").await;
    home.until_turns(&session, 3).await;

    let systems: Vec<String> = script
        .requests()
        .iter()
        .map(|(_, request)| request.system.clone())
        .collect();
    assert!(
        systems[0].ends_with("\n\n<alpha>A1</alpha>\n\n<bravo>B</bravo>"),
        "接在最后、照编号排：{}",
        systems[0]
    );
    assert!(
        !systems[0].contains("charlie") && !systems[0].contains("golf"),
        "没声明的、工具不在工具面上的不带"
    );
    assert_eq!(
        systems[1],
        systems[0].replace("<alpha>A1</alpha>", "<alpha>A2</alpha>"),
        "改了下一轮换上"
    );
    assert_eq!(
        systems[2],
        systems[0].replace("\n\n<alpha>A1</alpha>", ""),
        "去掉了和没放过的逐字节相同"
    );
    core.stop_extensions().await;
}
