//! 真跑 `miyu-index build`（施工 G-2 下）：测试里起一个很小的 HTTP 服务冒充 GitHub。收成一条写进 `index.json`、新占的编号
//! 写回 `names.toml`；令牌只发给 API 的地址，下包文件的不带。带 `--github` 的沿用上一份的上线时刻、数下载次数、写 `stats.json`
//! （施工 DL-2）。

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

fn path(dir: &Path) -> &str {
    dir.to_str().expect("UTF-8")
}

/// 照 `args` 跑 `miyu-index`，API 指到 `address`，令牌是 `tok`，不走代理。
fn run(address: &str, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_miyu-index"))
        .args(args)
        .env("MIYU_INDEX_API", address)
        .env("GITHUB_TOKEN", "tok")
        .env("NO_PROXY", "127.0.0.1")
        .env_remove("HTTP_PROXY")
        .env_remove("HTTPS_PROXY")
        .env_remove("ALL_PROXY")
        .env_remove("http_proxy")
        .env_remove("https_proxy")
        .env_remove("all_proxy")
        .output()
        .expect("跑得起来")
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

/// 下载量（施工 DL-2，设计 54 第二节）：带 `--github` 的沿用上一份的 `published`、照全部发布加下载次数、`stats.json` 记今天、
/// 近 7 天照 7 天前那天减；不带的只有 `published`，不问发布、不写 `stats.json`。
#[test]
fn build_with_github_carries_published_over_and_counts_downloads() {
    let dir = scratch("github");
    let folder = dir.join("src/memes");
    write(
        &folder.join("package.toml"),
        "[package]\nversion = \"1.0\"\nname = { en = \"Memes\" }\n",
    );
    let file = dir.join("memes-1.0.miyu");
    archive::pack(&folder, "memes", &file).expect("打得成");
    let routes: Routes = Arc::default();
    let (address, seen) = serve(Arc::clone(&routes));
    {
        let mut routes = routes.lock().expect("锁得上");
        routes.insert(
            "/files/memes-1.0.miyu".to_string(),
            std::fs::read(&file).expect("在"),
        );
        routes.insert(
            "/repos/miyu-agent/miyu-memes/releases/latest".to_string(),
            format!("{{\"assets\": [{{\"name\": \"memes-1.0.miyu\", \"browser_download_url\": \"{address}/files/memes-1.0.miyu\"}}]}}").into_bytes(),
        );
        routes.insert(
            "/repos/miyu-agent/miyu-memes/releases?per_page=100&page=1".to_string(),
            br#"[{"assets": [{"name": "memes-1.0.miyu", "download_count": 40}]}, {"assets": [{"name": "memes-0.9.miyu", "download_count": 2}]}]"#.to_vec(),
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
    let today = jiff::Timestamp::now()
        .to_zoned(jiff::tz::TimeZone::UTC)
        .date();
    let week_ago = today
        .checked_sub(jiff::Span::new().days(7))
        .expect("算得出");
    let out = dir.join("out");
    write(
        &out.join("index.json"),
        r#"{"version": 1, "generated": "2026-10-20T03:00:00Z", "packages": [{"id": "memes", "version": "1.0", "origin": "github.com/miyu-agent/miyu-memes", "published": "2026-10-20T03:00:00Z", "files": []}]}"#,
    );
    write(
        &out.join("stats.json"),
        &format!(
            r#"{{"version": 1, "days": [{{"date": "{week_ago}", "downloads": {{"memes": 12}}}}]}}"#
        ),
    );
    let ran = run(&address, &["build", path(&rules), path(&out), "--github"]);
    assert_eq!(ran.status.code(), Some(0), "{ran:?}");
    let index =
        Index::parse(&std::fs::read(out.join("index.json")).expect("写了")).expect("读得成");
    let memes = &index.packages[0];
    assert_eq!(
        (
            memes.published.as_deref(),
            memes.downloads,
            memes.downloads_week
        ),
        (Some("2026-10-20T03:00:00Z"), Some(42), Some(30)),
        "沿用上一份的上线时刻；各版本加起来；今天的减 7 天前的"
    );
    let stats: serde_json::Value =
        serde_json::from_slice(&std::fs::read(out.join("stats.json")).expect("写了"))
            .expect("读得成");
    assert_eq!(
        stats["days"],
        serde_json::json!([
            {"date": week_ago.to_string(), "downloads": {"memes": 12}},
            {"date": today.to_string(), "downloads": {"memes": 42}},
        ])
    );
    assert!(
        seen.lock().expect("锁得上").iter().any(|(path, auth)| path
            .starts_with("/repos/miyu-agent/miyu-memes/releases?")
            && auth.as_deref() == Some("Bearer tok")),
        "列发布带令牌"
    );
    let printed = format!(
        "{}{}{}",
        String::from_utf8_lossy(&ran.stdout),
        String::from_utf8_lossy(&ran.stderr),
        std::fs::read_to_string(out.join("stats.json")).expect("在")
    );
    assert!(!printed.contains("tok"), "令牌不进输出、不进 stats.json");

    // 问不成（限流、仓库没了）的照上一份沿用，不挡生成。
    routes
        .lock()
        .expect("锁得上")
        .remove("/repos/miyu-agent/miyu-memes/releases?per_page=100&page=1");
    let ran = run(&address, &["build", path(&rules), path(&out), "--github"]);
    assert_eq!(ran.status.code(), Some(0), "{ran:?}");
    let index =
        Index::parse(&std::fs::read(out.join("index.json")).expect("写了")).expect("读得成");
    assert_eq!(
        (
            index.packages[0].downloads,
            index.packages[0].downloads_week
        ),
        (Some(42), Some(30)),
        "{ran:?}"
    );
    assert!(
        String::from_utf8_lossy(&ran.stdout).contains("memes: downloads kept from the last index"),
        "{ran:?}"
    );

    let plain = dir.join("plain");
    std::fs::create_dir_all(&plain).expect("建得了");
    let asked = seen.lock().expect("锁得上").len();
    let ran = run(&address, &["build", path(&rules), path(&plain)]);
    assert_eq!(ran.status.code(), Some(0), "{ran:?}");
    let index =
        Index::parse(&std::fs::read(plain.join("index.json")).expect("写了")).expect("读得成");
    let memes = &index.packages[0];
    assert_eq!(
        (
            memes.published.as_deref(),
            memes.downloads,
            memes.downloads_week
        ),
        (Some(index.generated.as_str()), None, None),
        "没有上一份的写这一次的时刻；不带 --github 的没有下载次数"
    );
    assert!(!plain.join("stats.json").exists());
    assert!(
        !seen.lock().expect("锁得上")[asked..]
            .iter()
            .any(|(path, _)| path.contains("/releases?")),
        "不问发布"
    );
    assert!(std::fs::remove_dir_all(&dir).is_ok());
}
