//! 扩展用 embedding（施工 X-17，`model.embed`）：照这个账号的 `models.embedding` 算一批字的向量，交回模型编号和照先后的向量
//! （归一化过的）；
//! 一条一条问远程那一家；条数、字数不对的 `bad_params`；没接向量那一路的 `embedding_off`；那一家出错的 `embedding_failed`、
//! `data.why` 说原因。

use serde_json::{Value, json};

use miyu_http::testkit::{Reply, Server};
use miyu_session::testkit::Script;

use crate::support::embedding::{remote, vector};
use crate::support::*;

/// 拒绝的原因。
fn reason(reply: &Value) -> Option<&str> {
    reply["error"]["data"]["reason"].as_str()
}

#[tokio::test]
async fn texts_come_back_as_vectors_of_the_configured_model() {
    let server = Server::start(vec![vector(); 4]).await;
    let home = Home::new();
    let mut client = Client::connect(remote(&home, &server));
    client.hello().await;
    let reply = client
        .call("e1", "model.embed", json!({"texts": ["猫", "狗"]}))
        .await;
    assert_eq!(reply["result"]["model"], "emb/text-emb", "{reply}");
    let vectors: Vec<Vec<f64>> =
        serde_json::from_value(reply["result"]["vectors"].clone()).expect("是向量的数组");
    assert_eq!(vectors.len(), 2, "照先后一条一个");
    for vector in &vectors {
        let wanted = [0.6, 0.8, 0.0, 0.0];
        assert!(
            vector
                .iter()
                .zip(wanted)
                .all(|(got, want)| (got - want).abs() < 1e-6),
            "归一化过的：{vector:?}"
        );
    }
    assert_eq!(server.received().len(), 2, "一条一条问");
    assert!(server.received()[0].path.ends_with("/embeddings"));

    let long = "字".repeat(8001);
    let many: Vec<String> = (0..65).map(|n| n.to_string()).collect();
    for texts in [json!([]), json!([""]), json!([long]), json!(many)] {
        let bad = client
            .call("e2", "model.embed", json!({"texts": texts}))
            .await;
        assert_eq!(reason(&bad), Some("bad_params"), "{bad}");
    }
    assert_eq!(server.received().len(), 2, "写法不对的不问");
}

#[tokio::test]
async fn without_embedding_it_is_off_and_a_failing_provider_says_why() {
    let home = Home::new();
    let mut client = Client::connect(home.core(&Script::new([])));
    client.hello().await;
    let off = client
        .call("e1", "model.embed", json!({"texts": ["猫"]}))
        .await;
    assert_eq!(reason(&off), Some("embedding_off"), "{off}");
    // 接了向量那一路、没配远程也没有本机的：同样是关着。
    let home = Home::new();
    let core = home
        .core_full(&Script::new([]), miyu_tool::Catalog::default(), None, TOKEN)
        .with_vectors(None);
    let mut client = Client::connect(std::sync::Arc::new(core));
    client.hello().await;
    let none = client
        .call("e1", "model.embed", json!({"texts": ["猫"]}))
        .await;
    assert_eq!(reason(&none), Some("embedding_off"), "{none}");

    let broken = Reply {
        status: 500,
        headers: Vec::new(),
        body: Vec::new(),
    };
    let server = Server::start(vec![broken; 8]).await;
    let home = Home::new();
    let mut client = Client::connect(remote(&home, &server));
    client.hello().await;
    let failed = client
        .call("e2", "model.embed", json!({"texts": ["猫"]}))
        .await;
    assert_eq!(reason(&failed), Some("embedding_failed"), "{failed}");
    assert!(
        failed["error"]["data"]["why"]
            .as_str()
            .is_some_and(|why| !why.is_empty()),
        "{failed}"
    );
}
