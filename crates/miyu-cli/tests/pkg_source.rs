//! `miyu pkg update`、`-Sy`（施工 G-3 上）：在进程里起的核心认测试的钥匙，假服务器当软件源。拿到了印包数和索引的时刻；没拿成、
//! 用的是本机那份的说一句、退出码 1；一处都没有的照核心的原话、退出码 1。

use std::path::PathBuf;
use std::sync::Arc;

use clap::Parser;
use serde_json::json;

use crate::support::{Home, Tape, within};
use miyu_cli::language::Language;
use miyu_http::testkit::{Piece, Reply, Server};
use miyu_sandbox::Availability;
use miyu_session::testkit::Script;
use miyu_source::sign::{self, PublicKey};
use miyu_tool::Catalog;

/// 一份签好的索引的两份回应：索引、签名。
fn serving(generated: &str, count: usize, secret: &str) -> Vec<Reply> {
    let packages: Vec<serde_json::Value> = (0..count)
        .map(|n| {
            json!({"id": format!("p{n}"), "version": "1.0", "origin": "github.com/a/b",
                   "files": [{"url": "https://example.invalid/x.miyu", "sha256": "ab", "size": 1}]})
        })
        .collect();
    let index =
        serde_json::to_vec(&json!({"version": 1, "generated": generated, "packages": packages}))
            .expect("写得成");
    let signature =
        serde_json::to_vec(&sign::sign(&index, secret).expect("签得了")).expect("写得成");
    vec![
        Reply::stream(vec![Piece::Bytes(index)]),
        Reply::stream(vec![Piece::Bytes(signature)]),
    ]
}

/// 认 `keys`、系统配置里软件源是 `source` 的核心。
fn home(source: &str, keys: Vec<PublicKey>) -> Home {
    home_with_mirror(source, "", keys)
}

/// 同 [`home`]，另配一个镜像（空的不配）。
pub(crate) fn home_with_mirror(source: &str, mirror: &str, keys: Vec<PublicKey>) -> Home {
    let mirrors = if mirror.is_empty() {
        String::new()
    } else {
        format!("\"{mirror}\"")
    };
    home_written(
        format!("[packages]\nsource = \"{source}\"\nmirrors = [{mirrors}]\n"),
        keys,
    )
}

/// 认 `keys`、系统配置写的是 `config` 的核心。
fn home_written(config: String, keys: Vec<PublicKey>) -> Home {
    Home::shaped(
        Arc::new(Script::new([])),
        Catalog::default(),
        Availability::Usable(PathBuf::from("miyu-sandbox")),
        move |core, root| {
            let path = root.path().join("config.toml");
            std::fs::create_dir_all(path.parent().expect("有上一层")).expect("建得了");
            std::fs::write(&path, &config).expect("写得进");
            let config = miyu_endpoint::config::Config::load(
                root,
                None,
                [
                    miyu_endpoint::settings::SourceSettings::ITEMS,
                    miyu_endpoint::settings::SOURCE_FEEDS,
                ]
                .concat(),
                miyu_endpoint::config::Environment::of(&[]),
            );
            core.with_config(config).with_source_keys(keys)
        },
    )
}

/// 照 `words` 跑一次 `miyu pkg`：退出码、标准输出、标准错误。
async fn pkg(home: &Home, words: &[&str]) -> (u8, String, String) {
    pkg_answering(home, words, b"").await
}

/// 同 [`pkg`]，问的那一句照 `input` 答。
async fn pkg_answering(home: &Home, words: &[&str], input: &[u8]) -> (u8, String, String) {
    let (code, tape) = pkg_speaking(home, words, input, Language::Chinese).await;
    (code, tape.text(|err| !err), tape.text(|err| err))
}

/// 同 [`pkg_answering`]，照 `language` 说，交回退出码和录下的两路（施工 IN-3：标准输出、标准错误交错着看）。
pub(crate) async fn pkg_speaking(
    home: &Home,
    words: &[&str],
    input: &[u8],
    language: Language,
) -> (u8, Tape) {
    #[derive(Parser)]
    struct Line {
        #[command(subcommand)]
        command: Top,
    }
    #[derive(clap::Subcommand)]
    enum Top {
        Pkg(miyu_cli::Pkg),
    }
    let mut all = vec!["miyu", "pkg"];
    all.extend(words);
    let Top::Pkg(args) = Line::try_parse_from(all).expect("读得懂").command;
    let plan = miyu_cli::PkgPlan {
        args,
        language,
        cwd: home.outside(),
        terminal: None,
    };
    let (connection, token) = miyu_ipc::connect(&home.root).await.expect("连得上");
    let tape = Tape::default();
    let (mut out, mut err) = (tape.pen(false), tape.pen(true));
    let code = within(
        "pkg",
        miyu_cli::pkg_on(
            connection,
            &token,
            &plan,
            &mut &input[..],
            &mut out,
            &mut err,
        ),
    )
    .await;
    (code, tape)
}

pub(crate) fn keys() -> (String, Vec<PublicKey>) {
    let made = sign::generate().expect("生成得了");
    (
        made.secret,
        vec![PublicKey::from_base64(&made.public).expect("读得成")],
    )
}

#[tokio::test]
async fn update_fetches_the_index_and_says_how_many() {
    let (secret, trusted) = keys();
    let mut replies = serving("2026-10-19T03:00:00Z", 2, &secret);
    replies.extend(serving("2026-10-18T03:00:00Z", 0, &secret));
    let server = Server::start(replies).await;
    let home = home(&server.base_url, trusted);
    let updated = pkg(&home, &["update"]).await;
    assert_eq!(
        updated,
        (
            0,
            "已更新 official：2 个包（2026-10-19T03:00:00Z）\n".to_string(),
            String::new()
        )
    );
    let plain = pkg(&home, &["-S"]).await;
    assert_eq!(
        plain,
        (
            2,
            String::new(),
            "miyu: 没有指定目标\n提示：运行 'miyu pkg --help' 查看用法\n".to_string()
        ),
        "照 pacman 说没有指定目标"
    );
    let install = pkg(&home, &["-S", "embed", "--yes"]).await;
    assert_eq!(
        (install.0, install.2.as_str()),
        (1, "正在解析依赖…\n正在检查冲突…\nmiyu: 软件包不存在\n"),
        "-S <编号> 从软件源装，索引里没有的照核心的原话"
    );
    let stale = pkg(&home, &["-Sy"]).await;
    assert_eq!(stale.0, 1, "旧的不收、算没拿成：{stale:?}");
    assert!(
        stale.2.starts_with("miyu: 无法更新 official："),
        "{stale:?}"
    );
}

/// 配置里另加的软件源（施工 CL-2 补）：一个源一行，官方的在前，另加的照名字；另加的拿不成的警告一句，不算失败。
#[tokio::test]
async fn update_says_one_line_for_each_source() {
    let (secret, trusted) = keys();
    let theirs = sign::generate().expect("生成得了");
    let mut replies = serving("2026-10-19T03:00:00Z", 2, &secret);
    replies.extend(serving("2026-10-18T03:00:00Z", 3, &theirs.secret));
    let server = Server::start(replies).await;
    let base = &server.base_url;
    let public = &theirs.public;
    let home = home_written(
        format!(
            "[packages]\nsource = \"{base}\"\nmirrors = []\n\n\
             [packages.sources.extra]\nurl = \"https://unreachable.invalid/\"\nkey = \"{public}\"\n\n\
             [packages.sources.friend]\nurl = \"{base}\"\nkey = \"{public}\"\n"
        ),
        trusted,
    );
    let updated = pkg(&home, &["update"]).await;
    assert_eq!(
        updated,
        (
            0,
            "已更新 official：2 个包（2026-10-19T03:00:00Z）\n已更新 friend：3 个包（2026-10-18T03:00:00Z）\n"
                .to_string(),
            "miyu: 警告：无法更新 extra：无法连接\n".to_string()
        )
    );
}

#[tokio::test]
async fn update_with_no_source_and_no_index_is_refused() {
    let (_, trusted) = keys();
    let (other, _) = keys();
    let server = Server::start(serving("2026-10-19T03:00:00Z", 1, &other)).await;
    let home = home(&server.base_url, trusted);
    let refused = pkg(&home, &["update"]).await;
    assert_eq!(refused.0, 1, "{refused:?}");
    assert_eq!(refused.2, "miyu: 软件源连不上\n");
}

#[tokio::test]
async fn search_prints_one_line_each_and_nothing_found_is_exit_one() {
    let (secret, trusted) = keys();
    let index = serde_json::to_vec(&json!({
        "version": 1,
        "generated": "2026-10-19T03:00:00Z",
        "packages": [
            {"id": "embed", "version": "1.0", "name": {"zh": "语义模型"}, "summary": {"zh": "本机算向量"},
             "origin": "github.com/miyu-agent/miyu-embed", "official": true,
             "files": [{"url": "https://example.invalid/x.miyu", "sha256": "ab", "size": 1}]},
            {"id": "cat", "version": "1.0", "name": {"zh": "猫"},
             "origin": "github.com/someone/cat", "official": false,
             "files": [{"url": "https://example.invalid/y.miyu", "sha256": "ab", "size": 1}]},
        ],
        "blocked": [{"id": "cat", "reason": "spam"}],
    }))
    .expect("写得成");
    let signature =
        serde_json::to_vec(&sign::sign(&index, &secret).expect("签得了")).expect("写得成");
    let server = Server::start(vec![
        Reply::stream(vec![Piece::Bytes(index)]),
        Reply::stream(vec![Piece::Bytes(signature)]),
    ])
    .await;
    let home = home(&server.base_url, trusted);
    let all = pkg(&home, &["search"]).await;
    assert_eq!(
        all,
        (
            0,
            "embed  语义模型  本机算向量\ncat    猫（社区）（已拉黑：spam）\n".to_string(),
            String::new()
        )
    );
    let one = pkg(&home, &["-Ss", "向量"]).await;
    assert_eq!(one.1, "embed  语义模型  本机算向量\n", "{one:?}");
    let none = pkg(&home, &["search", "nothing"]).await;
    assert_eq!(
        none,
        (1, String::new(), String::new()),
        "什么都没搜到的退出码 1"
    );
}

/// 打一个包：编号、依赖，交回文件名、字节。
pub(crate) fn package(dir: &std::path::Path, id: &str, depends: &[&str]) -> (String, Vec<u8>) {
    let folder = dir.join("src").join(id);
    std::fs::create_dir_all(&folder).expect("建得了");
    let mut text = format!("[package]\nversion = \"1.0\"\nname = {{ en = \"{id}\" }}\n");
    if !depends.is_empty() {
        let depends: Vec<String> = depends.iter().map(|d| format!("\"{d}\"")).collect();
        text.push_str(&format!(
            "\n[depends]\npackages = [{}]\n",
            depends.join(", ")
        ));
    }
    std::fs::write(folder.join("package.toml"), text).expect("写得进");
    let name = miyu_source::archive::file_name(id, "1.0", None);
    let out = dir.join("out").join(&name);
    std::fs::create_dir_all(out.parent().expect("有上一层")).expect("建得了");
    miyu_source::archive::pack(&folder, id, &out).expect("打得成");
    (name, std::fs::read(&out).expect("在"))
}

pub(crate) fn entry(id: &str, depends: &[&str], file: &(String, Vec<u8>)) -> serde_json::Value {
    use sha2::Digest;
    let sha: String = sha2::Sha256::digest(&file.1)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    json!({"id": id, "version": "1.0", "name": {"en": id}, "origin": format!("github.com/miyu-agent/{id}"),
           "official": true, "depends": depends,
           "files": [{"url": format!("https://example.invalid/{}", file.0), "size": file.1.len(), "sha256": sha}]})
}

#[tokio::test]
async fn install_from_the_source_shows_the_plan_asks_and_cleans_up_after() {
    let (secret, trusted) = keys();
    let scratch = std::env::temp_dir().join(format!("miyu-cli-source-{}", std::process::id()));
    let app = package(&scratch, "app", &["lib"]);
    let lib = package(&scratch, "lib", &[]);
    let index = serde_json::to_vec(&json!({"version": 1, "generated": "2026-10-19T03:00:00Z",
        "packages": [entry("app", &["lib"], &app), entry("lib", &[], &lib)]}))
    .expect("写得成");
    let signature =
        serde_json::to_vec(&sign::sign(&index, &secret).expect("签得了")).expect("写得成");
    let server = Server::start(vec![
        Reply::stream(vec![Piece::Bytes(index)]),
        Reply::stream(vec![Piece::Bytes(signature)]),
        Reply::stream(vec![Piece::Bytes(lib.1.clone())]),
        Reply::stream(vec![Piece::Bytes(app.1.clone())]),
    ])
    .await;
    // 服务器当镜像：索引、包文件都先从它拿；软件源的原址到不了。
    let mirror = format!("{}/m", server.base_url);
    let home = home_with_mirror("https://unreachable.invalid/", &mirror, trusted);
    let declined = pkg_answering(&home, &["install", "app"], b"n\n").await;
    assert_eq!(declined.0, 1, "答不装的不装：{declined:?}");
    assert!(
        declined
            .1
            .starts_with("软件包 (2)  lib-1.0  app-1.0\n\n下载大小  : "),
        "{declined:?}"
    );
    assert!(
        declined
            .1
            .ends_with("来源      : 官方，签名已验证\n\n安装 2 个软件包？[Y/n] "),
        "{declined:?}"
    );
    assert_eq!(
        declined.2, "正在解析依赖…\n正在检查冲突…\n已取消\n",
        "{declined:?}"
    );
    let installed = pkg_answering(&home, &["-S", "app"], b"y\n").await;
    assert_eq!(installed.0, 0, "{installed:?}");
    assert!(
        installed.2.contains("正在下载…\n已下载 lib-1.0 "),
        "问完接着下载，管道里一个包一行：{installed:?}"
    );
    assert!(
        installed.1.ends_with("已安装 lib\n已安装 app 1.0\n"),
        "{installed:?}"
    );
    let cleaned = pkg(&home, &["clean"]).await;
    assert_eq!(
        cleaned,
        (0, "没有可清理的包文件\n".to_string(), String::new())
    );
    let cleaned = pkg(&home, &["-Sc"]).await;
    assert_eq!(cleaned.0, 0, "{cleaned:?}");
    if let Err(error) = std::fs::remove_dir_all(&scratch) {
        eprintln!("临时目录没删掉：{error}");
    }
}

/// 同 [`package`]，版本是 `version`。
fn package_at(dir: &std::path::Path, id: &str, version: &str) -> (String, Vec<u8>) {
    let folder = dir.join("src").join(format!("{id}-{version}")).join(id);
    std::fs::create_dir_all(&folder).expect("建得了");
    std::fs::write(
        folder.join("package.toml"),
        format!("[package]\nversion = \"{version}\"\nname = {{ en = \"{id}\" }}\n"),
    )
    .expect("写得进");
    let name = miyu_source::archive::file_name(id, version, None);
    let out = dir.join("out").join(&name);
    std::fs::create_dir_all(out.parent().expect("有上一层")).expect("建得了");
    miyu_source::archive::pack(&folder, id, &out).expect("打得成");
    (name, std::fs::read(&out).expect("在"))
}

/// 一份签好的索引（`generated` 那一刻，几个包），两份回应。
fn index_replies(generated: &str, entries: Vec<serde_json::Value>, secret: &str) -> Vec<Reply> {
    let index =
        serde_json::to_vec(&json!({"version": 1, "generated": generated, "packages": entries}))
            .expect("写得成");
    let signature =
        serde_json::to_vec(&sign::sign(&index, secret).expect("签得了")).expect("写得成");
    vec![
        Reply::stream(vec![Piece::Bytes(index)]),
        Reply::stream(vec![Piece::Bytes(signature)]),
    ]
}

#[tokio::test]
async fn upgrade_shows_what_changes_and_asks() {
    let (secret, trusted) = keys();
    let scratch = std::env::temp_dir().join(format!("miyu-cli-upgrade-{}", std::process::id()));
    let app1 = package_at(&scratch, "app", "1.0");
    let app2 = package_at(&scratch, "app", "2.0");
    let mut entry2 = entry("app", &[], &app2);
    entry2["version"] = json!("2.0");
    let mut replies = index_replies(
        "2026-10-19T03:00:00Z",
        vec![entry("app", &[], &app1)],
        &secret,
    );
    replies.push(Reply::stream(vec![Piece::Bytes(app1.1.clone())]));
    replies.extend(index_replies("2026-10-20T03:00:00Z", vec![entry2], &secret));
    replies.push(Reply::stream(vec![Piece::Bytes(app2.1.clone())]));
    let server = Server::start(replies).await;
    let mirror = format!("{}/m", server.base_url);
    let home = home_with_mirror("https://unreachable.invalid/", &mirror, trusted);
    let installed = pkg(&home, &["-S", "app", "--yes"]).await;
    assert_eq!(installed.0, 0, "{installed:?}");
    let upgraded = pkg_answering(&home, &["-Syu"], b"y\n").await;
    assert_eq!(upgraded.0, 0, "{upgraded:?}");
    assert!(
        upgraded.1.starts_with(
            "已更新 official：1 个包（2026-10-20T03:00:00Z）\n将升级 app 1.0 → 2.0\n下载："
        ),
        "{upgraded:?}"
    );
    assert!(upgraded.1.contains("升级 'app'？[Y/n] "), "{upgraded:?}");
    assert!(
        upgraded.1.ends_with("已升级 app 1.0 → 2.0\n"),
        "{upgraded:?}"
    );
    let again = pkg(&home, &["upgrade"]).await;
    assert_eq!(
        again,
        (0, "没有可升级的软件包\n".to_string(), String::new())
    );
    if let Err(error) = std::fs::remove_dir_all(&scratch) {
        eprintln!("临时目录没删掉：{error}");
    }
}
