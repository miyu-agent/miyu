//! 替成员下东西（施工 U23 下下）：回环、内网照闸拒，测试的口子打开「回环当公网」才连得上本机的假服务器；跳转每一跳重新过闸；
//! 主机名照闸解析好的地址钉住去连。

use std::net::{IpAddr, Ipv4Addr};
use std::time::Duration;

use super::*;
use crate::guard::Testing;
use crate::testkit::{Piece, Reply, Server};

const BUDGET: Duration = Duration::from_secs(10);

/// 回一段字的 200。
fn ok(body: &str) -> Reply {
    Reply::stream(vec![Piece::Bytes(body.as_bytes().to_vec())])
}

/// 跳到 `to` 的 302。
fn redirect(to: &str) -> Reply {
    Reply::error(302, &[("location", to)], "")
}

/// 回环当公网、不走代理的测试口子；`hosts` 是名字到地址的表。
fn loopback(hosts: &[(&str, IpAddr)]) -> Guarded {
    Guarded::testing(Testing {
        loopback_public: true,
        hosts: hosts
            .iter()
            .map(|(name, ip)| ((*name).to_string(), *ip))
            .collect(),
        proxy: None,
        no_proxy: String::new(),
    })
}

#[tokio::test]
async fn loopback_is_refused_unless_the_test_opens_it() {
    let server = Server::start(vec![ok("index"), ok("index")]).await;
    let url = format!("{}/index.json", server.base_url);
    let shut = Guarded::testing(Testing::default());
    assert_eq!(
        shut.get(&url, 1024, BUDGET).await,
        Err("not a public address".to_string())
    );
    assert!(server.received().is_empty(), "没连过去");
    assert_eq!(
        loopback(&[]).get(&url, 1024, BUDGET).await,
        Ok(b"index".to_vec())
    );
}

#[tokio::test]
async fn every_hop_of_a_redirect_passes_the_guard() {
    let server = Server::start(vec![redirect("http://10.0.0.1/index.json")]).await;
    let url = format!("{}/index.json", server.base_url);
    assert_eq!(
        loopback(&[]).get(&url, 1024, BUDGET).await,
        Err("not a public address".to_string()),
        "跳到内网的那一跳拒"
    );
    let server = Server::start(vec![redirect("/v1/moved"), ok("moved")]).await;
    let url = format!("{}/index.json", server.base_url);
    assert_eq!(
        loopback(&[]).get(&url, 1024, BUDGET).await,
        Ok(b"moved".to_vec())
    );
}

#[tokio::test]
async fn a_host_name_is_pinned_to_what_the_guard_resolved() {
    let server = Server::start(vec![ok("pinned")]).await;
    let port = server
        .base_url
        .trim_start_matches("http://127.0.0.1:")
        .trim_end_matches("/v1");
    let guarded = loopback(&[
        ("source.example", IpAddr::V4(Ipv4Addr::LOCALHOST)),
        ("inside.example", IpAddr::V4(Ipv4Addr::new(192, 168, 1, 2))),
    ]);
    let url = format!("http://source.example:{port}/v1/index.json");
    assert_eq!(
        guarded.get(&url, 1024, BUDGET).await,
        Ok(b"pinned".to_vec())
    );
    assert_eq!(
        guarded
            .get(&format!("http://inside.example:{port}/x"), 1024, BUDGET)
            .await,
        Err("not a public address".to_string()),
        "解析出来是内网的拒"
    );
    assert_eq!(
        guarded
            .get(&format!("http://nowhere.example:{port}/x"), 1024, BUDGET)
            .await,
        Err("the host could not be resolved".to_string()),
        "解析不出来又不走代理的连不上"
    );
}

#[tokio::test]
async fn shapes_and_limits_are_refused() {
    let guarded = loopback(&[]);
    for url in [
        "ftp://example.com/x",
        "http://user:pass@example.com/x",
        "http://localhost/x",
        "http://box.local/x",
    ] {
        assert_eq!(
            guarded.get(url, 1024, BUDGET).await,
            Err("not a public address".to_string()),
            "{url}"
        );
    }
    let server = Server::start(vec![ok("0123456789")]).await;
    let url = format!("{}/big", server.base_url);
    assert_eq!(
        guarded.get(&url, 4, BUDGET).await,
        Err("larger than 4 bytes".to_string())
    );
    let hops: Vec<Reply> = (0..=REDIRECTS).map(|_| redirect("/v1/again")).collect();
    let server = Server::start(hops).await;
    let url = format!("{}/again", server.base_url);
    assert_eq!(
        guarded.get(&url, 1024, BUDGET).await,
        Err("too many redirects".to_string())
    );
}
