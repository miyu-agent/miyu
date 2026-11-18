//! 登录时核心去连哪（施工 AU-006，审计 #006）：别的机器上的服务不能让核心连本机的端口；回答有大小上限、往来有时限。

use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use miyu_http::Proxy;
use miyu_mcp::oauth::{Challenge, Problem, Trouble, discover};

fn http() -> miyu_http::Client {
    miyu_http::client(Proxy::Off).expect("客户端")
}

fn pointing_to(url: String) -> Challenge {
    Challenge {
        resource_metadata: Some(url),
        scope: None,
        error: None,
    }
}

/// 修以前：别的机器上的服务在挑战里写 `resource_metadata="http://127.0.0.1:<端口>/…"`，核心照着去连本机的那个端口（也没有
/// 时限，对方不回就一直等）。现在别的机器上的服务指到本机的一律不认，一个字节都不发。
#[tokio::test]
async fn a_remote_server_cannot_point_the_login_at_this_machine() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("听得了");
    let port = listener.local_addr().expect("有端口").port();
    let challenge = pointing_to(format!("http://127.0.0.1:{port}/prm"));
    let found = tokio::time::timeout(
        Duration::from_secs(10),
        discover(&http(), "https://mcp.remote.example/mcp", &challenge),
    )
    .await
    .expect("当场答，不去连");
    assert_eq!(found.err(), Some(Trouble::Metadata(Problem::Insecure)));
    let knocked = tokio::time::timeout(Duration::from_millis(200), listener.accept()).await;
    assert!(knocked.is_err(), "没有连过来");
}

/// 一个只会回固定一段的本机服务：每个连接读一次请求，回 `answer`；交回端口。
async fn answering(answer: Vec<u8>) -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("听得了");
    let port = listener.local_addr().expect("有端口").port();
    tokio::spawn(async move {
        while let Ok((mut stream, _)) = listener.accept().await {
            let answer = answer.clone();
            tokio::spawn(async move {
                let mut buffer = [0u8; 4096];
                drop(stream.read(&mut buffer).await);
                drop(stream.write_all(&answer).await);
                drop(stream.shutdown().await);
            });
        }
    });
    port
}

/// 修以前：元数据整份读进内存，没有上限。现在超过上限的当场停。
#[tokio::test]
async fn an_oversized_answer_is_refused() {
    let pad = "x".repeat(2 * 1024 * 1024);
    let body = format!(r#"{{"authorization_servers":["http://127.0.0.1:1"],"pad":"{pad}"}}"#);
    let answer = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let port = answering(answer.into_bytes()).await;
    let server = format!("http://127.0.0.1:{port}/mcp");
    let found = discover(
        &http(),
        &server,
        &pointing_to(format!("http://127.0.0.1:{port}/prm")),
    )
    .await;
    assert_eq!(
        found.err(),
        Some(Trouble::Unreachable("answer too large".to_string()))
    );
}

/// 修以前：对方收了请求不回，登录就一直等着。现在到了时限报连不上（时钟停着、自己往前走，不真等）。
#[tokio::test(start_paused = true)]
async fn a_silent_server_times_out() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("听得了");
    let port = listener.local_addr().expect("有端口").port();
    // 收下连接、什么都不回。
    let held = tokio::spawn(async move {
        let mut kept = Vec::new();
        while let Ok((stream, _)) = listener.accept().await {
            kept.push(stream);
        }
    });
    let server = format!("http://127.0.0.1:{port}/mcp");
    let found = discover(
        &http(),
        &server,
        &pointing_to(format!("http://127.0.0.1:{port}/prm")),
    )
    .await;
    held.abort();
    assert!(matches!(found, Err(Trouble::Unreachable(_))), "{found:?}");
}

/// 审计 #018（施工 AU-P1）：浏览器常先多开一条连接放着不发（预连接），真正的回调走另一条。修以前听回调的一条一条读，先接下的
/// 那条不发东西就一直卡着，带授权码的那条排在后面，等满登录的时限才算失败。现在每条各读各的。
#[tokio::test]
async fn an_idle_preconnect_does_not_hold_up_the_real_callback() {
    let listener = miyu_mcp::oauth::listen(None).await.expect("听得了");
    let port = listener.port();
    let idle = tokio::net::TcpStream::connect(("127.0.0.1", port))
        .await
        .expect("连得上");
    let browser = tokio::spawn(async move {
        // 让听的那一头先接下空着的那条。
        tokio::time::sleep(Duration::from_millis(100)).await;
        let mut real = tokio::net::TcpStream::connect(("127.0.0.1", port))
            .await
            .expect("连得上");
        real.write_all(b"GET /callback?code=c1&state=s1 HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n")
            .await
            .expect("写得出");
        let mut answer = Vec::new();
        drop(real.read_to_end(&mut answer).await);
        answer
    });
    let got = tokio::time::timeout(Duration::from_secs(5), listener.next())
        .await
        .expect("空着的连接没把回调卡住")
        .expect("听得了");
    assert!(
        matches!(&got.0, miyu_mcp::oauth::Callback::Code { code, .. } if code == "c1"),
        "{:?}",
        got.0
    );
    got.1.send(true, "ok").await;
    let answer = browser.await.expect("浏览器那头没出错");
    assert!(
        answer.starts_with(b"HTTP/1.1 200"),
        "{}",
        String::from_utf8_lossy(&answer)
    );
    drop(idle);
}
