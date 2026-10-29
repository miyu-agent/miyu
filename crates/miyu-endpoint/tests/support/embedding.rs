//! 接上远程 embedding 的核心（施工 R-5 补写在 `memory_meaning.rs`，施工 K-2 挪出来：知识库照意思找的测试也用），和假服务器回的
//! 一个向量 `[3, 4, 0, 0]`（什么都挺像）。

use std::sync::Arc;

use serde_json::json;

use miyu_http::testkit::{Piece, Reply, Server};
use miyu_session::testkit::Script;
use miyu_tool::Catalog;

use super::{Home, TOKEN, providers};

/// 假服务器回的一个向量 `[3, 4, 0, 0]`，报了 5 个输入 token。
pub fn vector() -> Reply {
    Reply {
        status: 200,
        headers: vec![("Content-Type".to_string(), "application/json".to_string())],
        body: vec![Piece::Bytes(
            br#"{"data":[{"embedding":[3,4,0,0]}],"usage":{"prompt_tokens":5}}"#.to_vec(),
        )],
    }
}

/// 接上远程的核心：一家 `emb` 在假服务器上，key 照核心的环境变量取，`models.embedding` 指着它的 `text-emb`；没有本机的。
pub fn remote(home: &Home, server: &Server) -> Arc<miyu_endpoint::Core> {
    remote_with(home, server, &Script::new([]), Catalog::default())
}

/// 同 [`remote`]，她照剧本 `script` 说、工具面照目录 `tools`（施工 K-2：会话里她的 `kb_search`）。
pub fn remote_with(
    home: &Home,
    server: &Server,
    script: &Script,
    tools: Catalog,
) -> Arc<miyu_endpoint::Core> {
    home.write(
        "config.toml",
        &format!(
            "[persona]\ndefault = \"engineer\"\n\n[providers.emb]\ndriver = \"openai-chat\"\nbase_url = \"{}\"\nkey = {{ env = \"EMB_KEY\" }}\n\n[models]\nembedding = \"emb/text-emb\"\n",
            server.base_url
        ),
    );
    let items = [
        miyu_endpoint::settings::UiSettings::ITEMS,
        miyu_endpoint::settings::PersonaSettings::ITEMS,
        miyu_models::settings::UseSettings::ITEMS,
        miyu_models::settings::ProviderSettings::ITEMS,
        miyu_models::settings::ModelSettings::ITEMS,
    ]
    .concat();
    let config = miyu_endpoint::config::Config::load(
        &home.root,
        None,
        items,
        miyu_endpoint::config::Environment::of(&[("EMB_KEY", "sk-emb")]),
    );
    let data = providers::data(providers::profiles(json!({})));
    let core = home
        .core_full(script, tools, None, TOKEN)
        .with_config(config)
        .with_model_data(data)
        .with_vectors(None);
    Arc::new(core)
}
