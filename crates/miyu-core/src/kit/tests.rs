//! 端口问搜索来源的选项时照这一份家底答（施工 N-2 下）：被限额的那一家写冷却到什么时候，别的选项、别的配置项不写。

use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use miyu_endpoint::builtins::OptionState;
use miyu_store::resources::ResourceRoot;

use super::Kit;

/// 本机回环上一个什么都回 429 的假服务器，交回地址。
async fn limited() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("绑得上");
    let port = listener.local_addr().expect("有地址").port();
    tokio::spawn(async move {
        while let Ok((mut socket, _)) = listener.accept().await {
            tokio::spawn(async move {
                let mut buffer = [0u8; 4096];
                let _read = socket.read(&mut buffer).await;
                let _sent = socket
                    .write_all(b"HTTP/1.1 429 X\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
                    .await;
            });
        }
    });
    format!("http://127.0.0.1:{port}/exa")
}

#[tokio::test]
async fn the_search_source_option_tells_when_a_limited_source_comes_back() {
    let resources = ResourceRoot::at(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources"));
    let url = limited().await;
    let search = Arc::new(
        miyu_net::Search::new(resources.path())
            .testing(vec![("exa".to_string(), url)])
            .only(&["exa"]),
    );
    let kit = Kit::with_search(&resources, Arc::clone(&search));
    assert_eq!(
        kit.option_state("net.search.source", "exa"),
        None,
        "还没搜过"
    );

    let before = SystemTime::now();
    let query = miyu_net::Query {
        text: "rust".to_string(),
        ..miyu_net::Query::default()
    };
    assert!(
        search.search(&query, None).await.is_err(),
        "唯一的一家限额了"
    );

    let Some(OptionState::CoolingUntil(until)) = kit.option_state("net.search.source", "exa")
    else {
        panic!("限额的那一家冷却着");
    };
    assert!(until.duration_since(before).expect("在以后") >= Duration::from_secs(29 * 60));
    assert_eq!(
        kit.option_state("net.search.source", "parallel"),
        None,
        "别的选项不写"
    );
    assert_eq!(
        kit.option_state("ui.language", "exa"),
        None,
        "别的配置项不写"
    );
}
