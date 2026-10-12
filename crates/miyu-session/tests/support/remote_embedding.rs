//! 远程的 embedding 的假服务器怎么接（施工 R-5 补写在 `memory_remote.rs`，施工 K-2 挪出来：知识库照意思找的测试也用）：那一家
//! 回的向量都是 `[3, 4, 0, 0]`（没归一化过，什么都挺像）。

use std::sync::Arc;

use miyu_config::secret::Reference;
use miyu_http::testkit::{Piece, Reply, Server};
use miyu_models::matching::Vendors;
use miyu_models::profile::Profiles;
use miyu_session::{ModelData, Observed, Vectors};

use super::Home;

/// 向量记的模型编号。
pub const MODEL: &str = "emb/text-emb";

/// 200，JSON 的 `body`。
pub fn json(body: &str) -> Reply {
    Reply {
        status: 200,
        headers: vec![("Content-Type".to_string(), "application/json".to_string())],
        body: vec![Piece::Bytes(body.as_bytes().to_vec())],
    }
}

/// 那一家回的一个向量 `values`；`usage` 的报了 7 个输入 token（合计写 9，读的是输入那一格）。
pub fn answer(values: &str, usage: bool) -> Reply {
    let usage = match usage {
        true => r#","usage":{"prompt_tokens":7,"total_tokens":9}"#,
        false => "",
    };
    json(&format!(
        r#"{{"object":"list","data":[{{"object":"embedding","index":0,"embedding":{values}}}],"model":"text-emb"{usage}}}"#
    ))
}

/// 那一家回的 `[3, 4, 0, 0]`。
pub fn vector(usage: bool) -> Reply {
    answer("[3,4,0,0]", usage)
}

/// 场地接上远程的：一家 `emb` 在假服务器上，key 是 `sk-emb`，档案给它另配一个头 `x-trace`，`models.embedding` 指着它的
/// `text-emb`；没有本机的。
pub fn remote(home: &mut Home, server: &Server) {
    let source = format!(
        "[providers.emb]\ndriver = \"openai-chat\"\nbase_url = \"{}\"\nkey = {{ env = \"EMB_KEY\" }}\n\n[models]\nembedding = \"{MODEL}\"\n",
        server.base_url
    );
    home.configs = crate::support::routing::configs(
        &source,
        &[(Reference::Env("EMB_KEY".to_string()), "sk-emb")],
    );
    let profiles =
        serde_json::json!({"providers": {"emb": {"headers": {"x-trace": "t_{session_digest}"}}}});
    let data = ModelData::new(
        Profiles::parse(&profiles).expect("档案写法对"),
        Vendors::default(),
        None,
    )
    .with_fetcher(miyu_http::fetcher(miyu_http::Proxy::Off).expect("造得出"));
    data.loaded(None, Observed::default());
    data.keep_ledger(Arc::clone(&home.usage));
    let given = home
        .memory
        .give_vectors(Arc::new(Vectors::new(None, Arc::new(data))));
    assert!(given);
}
