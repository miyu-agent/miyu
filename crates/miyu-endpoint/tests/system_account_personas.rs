//! 系统账号的会话找人格（施工 Y-2 中上，核心主会话报的）：属主是桥的系统账号的群会话，照它服务的人（管理员）家目录里的人格找。

use miyu_session::testkit::Script;

use crate::support::extensions::*;
use crate::support::*;
use crate::system_account::{replies, served_core};
/// 系统账号的会话照它服务的人（管理员）家目录里的人格找（施工 Y-2 中上，核心主会话报的）：主人在自己家目录建的人格，群里照样
/// 找得到；照桥自己的家目录找的话这里回「没有这个人格」。
#[tokio::test]
async fn a_system_account_s_session_finds_personas_in_the_admin_s_home() {
    let home = Home::new();
    home.write(
        "home/alice/personas/house/prompts/persona.md",
        "House rules.\n",
    );
    let program = Program::new();
    let (path, step) = record(&home, "bot");
    let group = r#"{"venue":"qq:group:2","kind":"group","persona":"house"}"#;
    install_serving(
        &home,
        "bot",
        &program.name(),
        &steps(&[
            &step,
            "hello",
            &format!("ask:venue.session:{group}"),
            "wait",
        ]),
        "always",
        true,
    );
    let script = Script::new([]);
    let _core = served_core(&home, &script);
    let got = replies(&path, 2).await;
    assert_eq!(
        got[1]["result"]["created"], true,
        "主人家目录里的人格找得到：{got:?}"
    );
    assert_eq!(got[1]["result"]["account"], "bot", "会话照旧归系统账号");
}
