//! 回合开始问扩展要加的字（施工 X-16，`docs/blueprint/extensions.md`「回合开始的挂接点」）：声明了 `context.inject`、在跑、
//! 工具面上有它的工具的扩展被问，交回的记成 `context.injected`、`by` 是这个包，排在人说的话前面；没声明的、工具不在这个会话的
//! 工具面上的不问；超时、回错误、写法不对、交多了的当没交，这一轮照常走完。

use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};

use miyu_kernel::event::Body;
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

/// 一个扩展：编号、怎么答 `turn.start`、声明不声明 `context.inject`、工具在哪种场所。
struct Ext {
    id: &'static str,
    inject: &'static str,
    declares: bool,
    venue: &'static str,
}

const EXTS: [Ext; 8] = [
    Ext {
        id: "alpha",
        inject: "hint",
        declares: true,
        venue: "local",
    },
    Ext {
        id: "bravo",
        inject: "silent",
        declares: true,
        venue: "local",
    },
    Ext {
        id: "charlie",
        inject: "garbled",
        declares: true,
        venue: "local",
    },
    Ext {
        id: "delta",
        inject: "many",
        declares: true,
        venue: "local",
    },
    Ext {
        id: "echo",
        inject: "boom",
        declares: true,
        venue: "local",
    },
    // 没声明：答得出也不问。
    Ext {
        id: "foxtrot",
        inject: "hint",
        declares: false,
        venue: "local",
    },
    // 工具只在群里：本机的会话工具面上没有它，不问。
    Ext {
        id: "golf",
        inject: "hint",
        declares: true,
        venue: "group",
    },
    Ext {
        id: "hotel",
        inject: "hint",
        declares: true,
        venue: "local",
    },
];

#[tokio::test]
async fn extensions_add_blocks_before_what_was_said() {
    let home = Home::new();
    let programs: Vec<Program> = EXTS.iter().map(|_| Program::new()).collect();
    let mut records = Vec::new();
    let mut approved = serde_json::Map::new();
    for (ext, program) in EXTS.iter().zip(&programs) {
        let (path, step) = record(&home, ext.id);
        let capabilities: &[&str] = if ext.declares {
            &["tools", "context.inject"]
        } else {
            &["tools"]
        };
        install_asking(
            &home,
            ext.id,
            &program.name(),
            "always",
            &steps(&[
                &step,
                "hello",
                &format!("ask:provide:{}", tools(ext.id, ext.venue)),
                &format!("inject:{}", ext.inject),
                "serve",
            ]),
            capabilities,
        );
        approved.insert(ext.id.to_string(), json!(capabilities));
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
    let script = Script::new([Play::Says("好。")]).titles([Play::Says("测试")]);
    let core = Arc::new(
        home.core_full(&script, Catalog::default(), None, TOKEN)
            .with_extension_timing(quick()),
    );
    core.start_extensions();
    for path in &records {
        lines(path, 3).await;
    }
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let session = client.create("c1", "~").await;
    client.say("s1", &session, "早上好").await;
    home.until_turns(&session, 1).await;

    let injected: Vec<(String, String)> = home
        .log(&session)
        .iter()
        .filter_map(|event| match &event.body {
            Body::ContextInjected(fact) if fact.kind.as_str() == "meme" => {
                Some((format!("{:?}", event.by), fact.text.clone()))
            }
            _ => None,
        })
        .collect();
    assert_eq!(injected.len(), 2, "只有 alpha、hotel 交的：{injected:?}");
    assert!(
        injected[0].0.contains("alpha") && injected[1].0.contains("hotel"),
        "照编号排、by 是包：{injected:?}"
    );
    assert!(
        injected.iter().all(|(_, text)| text == "hint local 早上好"),
        "{injected:?}"
    );

    let request = format!("{:?}", script.requests()[0].1);
    let hint = request.find("hint local 早上好").expect("进了请求");
    let said = request.find("\"早上好\"").expect("人说的话在");
    assert!(hint < said, "排在人说的话前面");

    for (ext, path) in EXTS.iter().zip(&records) {
        let asked = read(path).contains("\"method\":\"turn.start\"");
        let expected = ext.declares && ext.venue == "local";
        assert_eq!(asked, expected, "{} 问没问", ext.id);
    }
    core.stop_extensions().await;
}
