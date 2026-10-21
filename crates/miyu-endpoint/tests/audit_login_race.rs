//! 审计复现（漏洞排查 2026-10-22）：密码登录的限流是「先查后记」——`throttled()` 查计数和 `failed()` 记失败之间隔着
//! argon2 验证，同时进来的握手都看到「还没错满 5 次」，都去验密码。60 秒 5 次的上限挡不住并发：攻击者一批开 N 个连接，
//! 一分钟就能试 N 个密码，而且每个都占一份 19 MiB 的 argon2 内存。
//!
//! 期望（安全的样子）：同一个用户名，60 秒内不管怎么并发，至多 5 次真的去验密码、回 `bad_password`，其余回
//! `login_throttled`。修之前这条是红的。

use std::sync::Arc;

use serde_json::json;
use tokio::task::JoinSet;

use miyu_session::testkit::Script;

use crate::support::login::*;
use crate::support::*;

#[tokio::test]
async fn concurrent_wrong_passwords_are_still_held_to_five_a_minute() {
    let home = Home::new();
    let core = home.core(&Script::new([]));
    set_up(&core, "shorin", "correct horse").await;
    let mut tries = JoinSet::new();
    for n in 0..20 {
        let core = Arc::clone(&core);
        tries.spawn(async move {
            let mut client = Client::connect(core);
            let reply = hello_with(
                &mut client,
                json!({"user": "shorin", "password": format!("guess number {n}")}),
            )
            .await;
            why(&reply).to_string()
        });
    }
    let mut verified = 0;
    let mut throttled = 0;
    while let Some(reason) = tries.join_next().await {
        match reason.expect("没崩").as_str() {
            "bad_password" => verified += 1,
            "login_throttled" => throttled += 1,
            other => panic!("没想到的回法：{other}"),
        }
    }
    assert!(
        verified <= 5,
        "20 个并发的错密码里有 {verified} 个真的去验了（限流的 {throttled} 个）：并发绕过了 60 秒 5 次"
    );
}

/// 对了清零（施工 5-20）：进来时先记一次失败，验对了要清掉。错 4 次、对 1 次，之后再错 4 次都还是真的去验。
#[tokio::test]
async fn a_right_password_clears_the_count() {
    let home = Home::new();
    let core = home.core(&Script::new([]));
    set_up(&core, "shorin", "correct horse").await;
    let attempt = |password: &'static str| {
        let core = Arc::clone(&core);
        async move {
            let mut client = Client::connect(core);
            hello_with(&mut client, json!({"user": "shorin", "password": password})).await
        }
    };
    for _ in 0..4 {
        assert_eq!(why(&attempt("wrong").await), "bad_password");
    }
    let right = attempt("correct horse").await;
    assert!(right.get("result").is_some(), "{right}");
    for n in 0..4 {
        let reply = attempt("wrong").await;
        assert_eq!(why(&reply), "bad_password", "对了以后第 {n} 次：{reply}");
    }
}
