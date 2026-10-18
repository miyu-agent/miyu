//! 真跑 `miyu-index build`（施工 G-2 下）：测试里起一个很小的 HTTP 服务冒充 GitHub。收成一条写进 `index.json`、新占的编号
//! 写回 `names.toml`；令牌只发给 API 的地址，下包文件的不带。

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};

use miyu_source::archive;
use miyu_source::index::Index;

/// 收到的请求：路径和 `authorization` 头。
type Seen = Arc<Mutex<Vec<(String, Option<String>)>>>;

/// 路径到内容：起好服务以后还能往里放（下载地址要带服务自己的地址）。
type Routes = Arc<Mutex<BTreeMap<String, Vec<u8>>>>;

/// 起一个只认 `routes` 里的路径的服务，交回地址和收到的请求。
fn serve(routes: Routes) -> (String, Seen) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("占得到端口");
    let address = format!("http://{}", listener.local_addr().expect("有地址"));
    let seen: Seen = Arc::default();
    let log = Arc::clone(&seen);
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let mut reader = BufReader::new(stream.try_clone().expect("复制得了"));
            let mut first = String::new();
            if reader.read_line(&mut first).is_err() {
                continue;
            }
            let path = first
                .split_whitespace()
                .nth(1)
                .unwrap_or_default()
                .to_string();
            let mut auth = None;
            loop {
                let mut line = String::new();
                if reader.read_line(&mut line).unwrap_or(0) == 0 || line == "\r\n" {
                    break;
                }
                if let Some((name, value)) = line.split_once(':')
                    && name.eq_ignore_ascii_case("authorization")
                {
                    auth = Some(value.trim().to_string());
                }
            }
            log.lock().expect("锁得上").push((path.clone(), auth));
            let found = routes.lock().expect("锁得上").get(&path).cloned();
            let (status, body) = match found {
                Some(body) => ("200 OK", body),
                None => ("404 Not Found", Vec::new()),
            };
            let mut stream = stream;
            let head = format!(
                "HTTP/1.1 {status}\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
                body.len()
            );
            if stream
                .write_all(head.as_bytes())
                .and_then(|()| stream.write_all(&body))
                .is_err()
            {
                continue;
            }
        }
    });
    (address, seen)
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("miyu-index-build-{name}-{}", std::process::id()));
    if dir.exists() {
        std::fs::remove_dir_all(&dir).expect("删得掉");
    }
    std::fs::create_dir_all(&dir).expect("建得了");
    std::fs::canonicalize(&dir).expect("在")
}

fn write(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().expect("有上一层")).expect("建得了");
    std::fs::write(path, text).expect("写得进");
}

#[test]
fn build_collects_an_official_release_into_an_index() {
    let dir = scratch("one");
    let folder = dir.join("src/memes");
    write(
        &folder.join("package.toml"),
        "[package]\nversion = \"1.0\"\nname = { en = \"Memes\" }\n",
    );
    let file = dir.join("memes-1.0.miyu");
    let packed = archive::pack(&folder, "memes", &file).expect("打得成");
    let bytes = std::fs::read(&file).expect("在");

    let routes: Routes = Arc::default();
    let (address, seen) = serve(Arc::clone(&routes));
    let release = format!(
        "{{\"assets\": [{{\"name\": \"memes-1.0.miyu\", \"browser_download_url\": \"{address}/files/memes-1.0.miyu\"}}]}}"
    );
    {
        let mut routes = routes.lock().expect("锁得上");
        routes.insert("/files/memes-1.0.miyu".to_string(), bytes);
        routes.insert(
            "/repos/miyu-agent/miyu-memes/releases/latest".to_string(),
            release.into_bytes(),
        );
    }
    let rules = dir.join("rules");
    write(
        &rules.join("sources.toml"),
        "[[source]]\nkind = \"official\"\n",
    );
    write(
        &rules.join("official.toml"),
        "repos = [\"miyu-agent/miyu-memes\"]\n",
    );
    let out = dir.join("out");
    std::fs::create_dir_all(&out).expect("建得了");
    let ran = Command::new(env!("CARGO_BIN_EXE_miyu-index"))
        .args([
            "build",
            rules.to_str().expect("UTF-8"),
            out.to_str().expect("UTF-8"),
        ])
        .env("MIYU_INDEX_API", &address)
        .env("GITHUB_TOKEN", "tok")
        .env("NO_PROXY", "127.0.0.1")
        .env_remove("HTTP_PROXY")
        .env_remove("HTTPS_PROXY")
        .env_remove("ALL_PROXY")
        .env_remove("http_proxy")
        .env_remove("https_proxy")
        .env_remove("all_proxy")
        .output()
        .expect("跑得起来");
    let said = String::from_utf8_lossy(&ran.stdout).into_owned();
    assert_eq!(ran.status.code(), Some(0), "{ran:?}");
    assert!(
        said.contains("miyu-agent/miyu-memes: claimed id memes"),
        "{said}"
    );
    assert!(said.contains("1 packages into"), "{said}");
    let index =
        Index::parse(&std::fs::read(out.join("index.json")).expect("写了")).expect("读得成");
    assert_eq!(index.packages[0].files[0].sha256, packed.sha256);
    assert!(!out.join(".collect").exists(), "解包用的删了");
    assert_eq!(
        std::fs::read_to_string(rules.join("names.toml"))
            .expect("写回去了")
            .lines()
            .last(),
        Some("memes = \"github.com/miyu-agent/miyu-memes\"")
    );
    let seen = seen.lock().expect("锁得上").clone();
    assert!(
        seen.iter().any(
            |(path, auth)| path.starts_with("/repos/") && auth.as_deref() == Some("Bearer tok")
        ),
        "API 带令牌：{seen:?}"
    );
    assert!(
        seen.iter()
            .any(|(path, auth)| path.starts_with("/files/") && auth.is_none()),
        "下包文件的不带令牌：{seen:?}"
    );
    assert!(std::fs::remove_dir_all(&dir).is_ok());
}
