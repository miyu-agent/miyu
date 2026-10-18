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
    let source = source.to_string();
    Home::shaped(
        Arc::new(Script::new([])),
        Catalog::default(),
        Availability::Usable(PathBuf::from("miyu-sandbox")),
        move |core, root| {
            let path = root.path().join("system/config.toml");
            std::fs::create_dir_all(path.parent().expect("有上一层")).expect("建得了");
            std::fs::write(&path, format!("[packages]\nsource = \"{source}\"\n")).expect("写得进");
            let config = miyu_endpoint::config::Config::load(
                root,
                &crate::support::AccountIdOf::admin(),
                None,
                miyu_endpoint::settings::SourceSettings::ITEMS.to_vec(),
                miyu_endpoint::config::Environment::of(&[]),
            );
            core.with_config(config).with_source_keys(keys)
        },
    )
}

/// 照 `words` 跑一次 `miyu pkg`：退出码、标准输出、标准错误。
async fn pkg(home: &Home, words: &[&str]) -> (u8, String, String) {
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
        language: Language::Chinese,
        cwd: home.outside(),
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
            &mut std::io::empty(),
            &mut out,
            &mut err,
        ),
    )
    .await;
    (code, tape.text(|err| !err), tape.text(|err| err))
}

fn keys() -> (String, Vec<PublicKey>) {
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
            "软件源已更新：2 个包（2026-10-19T03:00:00Z）\n".to_string(),
            String::new()
        )
    );
    let plain = pkg(&home, &["-S"]).await;
    assert_eq!(
        plain,
        (
            2,
            String::new(),
            "错误：没有指定目标（使用 -h 获取帮助）\n".to_string()
        ),
        "照 pacman 说没有指定目标"
    );
    let install = pkg(&home, &["-S", "embed"]).await;
    assert_eq!(
        install,
        (
            2,
            String::new(),
            "错误：还不能从软件源装（使用 -h 获取帮助）\n".to_string()
        )
    );
    let stale = pkg(&home, &["-Sy"]).await;
    assert_eq!(stale.0, 1, "旧的不收、算没拿成：{stale:?}");
    assert!(
        stale
            .2
            .starts_with("软件源连不上，用的是本机的索引（2026-10-19T03:00:00Z）："),
        "{stale:?}"
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
    assert_eq!(refused.2, "软件源连不上。\n");
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
            "embed  语义模型  本机算向量\ncat    猫（社区）（被拉黑：spam）\n".to_string(),
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
