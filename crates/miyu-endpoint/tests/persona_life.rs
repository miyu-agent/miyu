//! 人格的推演方法经协议读写（施工 LF-8 上，`docs/blueprint/personas.md` 第 6 条、「改」）：真核心走一遍。`persona.read`
//! 认 `life`，回叠好的原文、你那一层的版本，另带出厂的那一份 `default`（读不出的是 `null`）；`persona.set` 写你那一层的
//! `prompts/life.md`，照版本防覆盖，不认 `pairs`；`persona.get` 的 `prompts.life` 是有没有字。别的提示词名不带 `default`。
//! 「下面一层」用装着的样本包那一层验：出厂的资源目录是源码树，测试不往里放文件；两层在 `file_of` 里走同一条路。

use std::path::PathBuf;

use serde_json::{Value, json};

use crate::support::*;
use miyu_session::testkit::{SAMPLE_PERSONA, Script};

/// 出厂的推演方法，去掉末尾的空白。
fn shipped() -> &'static str {
    include_str!("../../../resources/software/life/method.txt").trim_end()
}

async fn connected(home: &Home) -> Client {
    let mut client = Client::connect(home.core(&Script::new([])));
    client.hello().await;
    client
}

/// 样本人格在下面那一层（装着的样本包里），连上照数据根读包的核心。
async fn below(home: &Home) -> Client {
    crate::support::packaged::sample_persona_below(home);
    let mut client = Client::connect(crate::support::packaged::core(home));
    client.hello().await;
    client
}

/// 样本包里样本人格的推演方法。
fn packaged_life(home: &Home) -> PathBuf {
    home.root
        .packages()
        .join(crate::support::packaged::SAMPLES)
        .join("personas")
        .join(SAMPLE_PERSONA)
        .join("prompts")
        .join("life.md")
}

/// 你那一层里人格 `id` 的推演方法。
fn my_life(home: &Home, id: &str) -> PathBuf {
    home.root
        .path()
        .join(format!("root/personas/{id}/prompts/life.md"))
}

async fn read(client: &mut Client, id: &str, prompt: &str) -> Value {
    let reply = client
        .call(
            id,
            "persona.read",
            json!({"persona": SAMPLE_PERSONA, "prompt": prompt}),
        )
        .await;
    assert!(reply.get("error").is_none(), "{reply}");
    reply["result"].clone()
}

async fn has_life(client: &mut Client, id: &str) -> Value {
    let got = client
        .call(id, "persona.get", json!({"persona": SAMPLE_PERSONA}))
        .await;
    got["result"]["prompts"]["life"].clone()
}

async fn set(client: &mut Client, id: &str, life: Value) -> Value {
    client
        .call(
            id,
            "persona.set",
            json!({"persona": SAMPLE_PERSONA, "prompts": {"life": life}}),
        )
        .await
}

#[tokio::test]
async fn reading_life_gives_the_layered_text_my_version_and_the_shipped_default() {
    let home = Home::new();
    let mut client = below(&home).await;
    let nothing = read(&mut client, "r1", "life").await;
    assert_eq!(
        nothing,
        json!({"text": null, "version": null, "default": shipped()}),
        "哪一层都没有"
    );
    assert_eq!(has_life(&mut client, "g1").await, false);
    for (n, prompt) in ["persona", "examples", "reminders"].iter().enumerate() {
        let other = read(&mut client, &format!("o{n}"), prompt).await;
        assert!(other.get("default").is_none(), "{prompt}：{other}");
    }

    std::fs::create_dir_all(packaged_life(&home).parent().unwrap()).unwrap();
    std::fs::write(packaged_life(&home), "包里的过法。\n").unwrap();
    assert_eq!(
        read(&mut client, "r2", "life").await,
        json!({"text": "包里的过法。\n", "version": null, "default": shipped()}),
        "下面那一层的原文，你那一层没有版本"
    );
    assert_eq!(has_life(&mut client, "g2").await, true);

    home.write(
        &format!("root/personas/{SAMPLE_PERSONA}/prompts/life.md"),
        "",
    );
    let empty = read(&mut client, "r3", "life").await;
    assert_eq!(
        empty["text"], "",
        "空文件盖住下面的，是空的字不是 null：{empty}"
    );
    assert!(empty["version"].is_string(), "{empty}");
    assert_eq!(empty["default"], shipped(), "{empty}");
    assert_eq!(has_life(&mut client, "g3").await, false, "空的算没有字");

    home.write(
        &format!("root/personas/{SAMPLE_PERSONA}/prompts/life.md"),
        "我的过法。\n",
    );
    let mine = read(&mut client, "r4", "life").await;
    assert_eq!(mine["text"], "我的过法。\n", "{mine}");
    assert!(mine["version"].is_string(), "{mine}");
    assert_ne!(mine["version"], empty["version"], "改了换版本");
    assert_eq!(has_life(&mut client, "g4").await, true);
}

#[tokio::test]
async fn the_default_is_null_when_the_shipped_method_cannot_be_read() {
    let home = Home::new();
    let resources = home.work.join("bare-resources");
    std::fs::create_dir_all(&resources).expect("建得了目录");
    let mut client = Client::connect(home.core_with_resources(&Script::new([]), resources));
    client.hello().await;
    assert_eq!(
        read(&mut client, "r1", "life").await,
        json!({"text": null, "version": null, "default": null}),
        "读不出出厂的那一份不算错"
    );
}

#[tokio::test]
async fn setting_life_writes_my_layer_and_guards_with_expect() {
    let home = Home::new();
    let mut client = below(&home).await;
    std::fs::create_dir_all(packaged_life(&home).parent().unwrap()).unwrap();
    std::fs::write(packaged_life(&home), "包里的过法。\n").unwrap();
    let path = my_life(&home, SAMPLE_PERSONA);

    let saved = set(
        &mut client,
        "s1",
        json!({"text": "第一版。\n", "expect": null}),
    )
    .await;
    assert!(saved.get("error").is_none(), "{saved}");
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "第一版。\n");
    assert_eq!(
        std::fs::read_to_string(packaged_life(&home)).unwrap(),
        "包里的过法。\n",
        "下面那一层不动"
    );
    let first = read(&mut client, "r1", "life").await;
    assert_eq!(first["text"], "第一版。\n", "{first}");
    let version = first["version"].clone();
    assert!(version.is_string(), "{first}");

    let stale = set(
        &mut client,
        "s2",
        json!({"text": "丢了。\n", "expect": null}),
    )
    .await;
    assert_eq!(reason(&stale), Some("persona_conflict"), "{stale}");
    let wrong = set(
        &mut client,
        "s3",
        json!({"text": "丢了。\n", "expect": "0000"}),
    )
    .await;
    assert_eq!(reason(&wrong), Some("persona_conflict"), "{wrong}");
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "第一版。\n",
        "文件没变"
    );

    let second = set(
        &mut client,
        "s4",
        json!({"text": "第二版。\n", "expect": version}),
    )
    .await;
    assert!(second.get("error").is_none(), "{second}");
    assert_eq!(read(&mut client, "r2", "life").await["text"], "第二版。\n");

    let unset = set(&mut client, "s5", json!({"unset": true})).await;
    assert!(unset.get("error").is_none(), "{unset}");
    assert!(!path.exists(), "删掉你那一层的");
    assert_eq!(
        read(&mut client, "r3", "life").await,
        json!({"text": "包里的过法。\n", "version": null, "default": shipped()}),
        "回到下面那一层的"
    );
    std::fs::remove_file(packaged_life(&home)).unwrap();
    assert_eq!(
        read(&mut client, "r4", "life").await["text"],
        Value::Null,
        "下面也没有就没有"
    );
}

#[tokio::test]
async fn get_tells_whether_life_has_words_and_set_answers_with_it() {
    let home = Home::new();
    let mut client = connected(&home).await;
    assert_eq!(has_life(&mut client, "g1").await, false);
    let written = set(&mut client, "s1", json!({"text": "照这样过。\n"})).await;
    assert_eq!(written["result"]["prompts"]["life"], true, "{written}");
    assert_eq!(has_life(&mut client, "g2").await, true);
    let blank = set(&mut client, "s2", json!({"text": " \n\n"})).await;
    assert_eq!(
        blank["result"]["prompts"]["life"], false,
        "只有空白的：{blank}"
    );
    assert_eq!(has_life(&mut client, "g3").await, false);

    let made = client
        .call(
            "s3",
            "persona.set",
            json!({"prompts": {"life": {"text": "新人格的过法。\n"}}}),
        )
        .await;
    assert_eq!(made["result"]["prompts"]["life"], true, "{made}");
    let id = made["result"]["persona"].as_str().expect("起了编号");
    assert_eq!(
        std::fs::read_to_string(my_life(&home, id)).unwrap(),
        "新人格的过法。\n",
        "新建时一起带上"
    );
}

#[tokio::test]
async fn life_with_pairs_is_refused_and_nothing_is_written() {
    let home = Home::new();
    let mut client = connected(&home).await;
    let refused = client
        .call(
            "s1",
            "persona.set",
            json!({
                "persona": SAMPLE_PERSONA,
                "prompts": {
                    "persona": {"text": "Changed.\n"},
                    "life": {"pairs": [{"user": "a", "assistant": "b"}]},
                },
            }),
        )
        .await;
    assert_eq!(reason(&refused), Some("bad_params"), "{refused}");
    assert!(!my_life(&home, SAMPLE_PERSONA).exists());
    let persona = read(&mut client, "r1", "persona").await;
    assert_ne!(persona["text"], "Changed.\n", "一起写的也没写：{persona}");
}
