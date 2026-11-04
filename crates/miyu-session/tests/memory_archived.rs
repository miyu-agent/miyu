//! 导进来的只读存档（施工 SE-1 下补，设计 58 第二节第 8 条，`docs/blueprint/memory.md` 第六条）：存档载入了照样接回合库，不抽取。
//!
//! 一个会话聊了两轮、还没抽过，核心这时才交抽取要的几样（闲一眨眼就抽），载入以后再落一轮（协议上存档不收新的话，这里绕过
//! 端点直接交给会话，守的是记忆这一侧）：不是存档的闲下来照常抽（对照）；会话目录里放了存档标记的，闲多久都不发请求、没有抽到哪。

use std::time::Duration;

use miyu_http::testkit::Server;
use miyu_kernel::id::SessionId;
use miyu_session::testkit::{Play, Script};
use miyu_store::bundle::{MARKER, Marker};

use crate::support::extracting::*;
use crate::support::meaning::{catalog, chat};
use crate::support::*;

fn says(n: usize) -> Script {
    Script::new((0..n).map(|_| Play::Says("好。")))
}

/// 没交抽取的核心上聊两轮、停下，再交抽取（闲 100 毫秒就抽）：交回这个会话，一轮都还没抽。
async fn unextracted(home: &mut Home, server: &Server) -> SessionId {
    let handle = home
        .create_full(
            &says(2),
            &catalog(home),
            Opening::default(),
            Lines::default(),
        )
        .await;
    let session = handle.id().clone();
    chat(home, &handle, 1, "我养了一只猫").await;
    chat(home, &handle, 2, "它叫团子").await;
    stop(&handle).await;
    organizer(home, server, "");
    merged_just_now(home);
    assert_eq!(mark(home, &session), None, "还没抽过");
    session
}

#[tokio::test]
async fn a_loaded_session_is_extracted_once_it_idles() {
    let server = Server::start(vec![every_turn("用户养了一只猫")]).await;
    let mut home = Home::new();
    let session = unextracted(&mut home, &server).await;
    let handle = home.load_with(&session, &says(1), &catalog(&home)).await;
    chat(&home, &handle, 3, "团子三岁了").await;
    requested(&server, 1).await;
    stop(&handle).await;
}

#[tokio::test]
async fn a_loaded_archive_is_never_extracted() {
    let server = Server::start(vec![every_turn("用户养了一只猫")]).await;
    let mut home = Home::new();
    let session = unextracted(&mut home, &server).await;
    let marker = Marker {
        archive: true,
        miyu: "test".to_string(),
        exported: "2026-11-04T00:00:00Z".to_string(),
    };
    std::fs::write(
        home.root.session_dir(&session).join(MARKER),
        serde_json::to_vec(&marker).expect("写得成"),
    )
    .expect("写得进");
    let handle = home.load_with(&session, &says(1), &catalog(&home)).await;
    chat(&home, &handle, 3, "团子三岁了").await;
    tokio::time::sleep(Duration::from_secs(2)).await;
    assert!(server.received().is_empty(), "存档不抽取");
    assert_eq!(mark(&home, &session), None);
    stop(&handle).await;
}
