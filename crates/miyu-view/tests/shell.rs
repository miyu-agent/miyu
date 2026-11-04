//! 人用 `!` 打的命令（施工 SH-1 下，设计 57 第四节第 4 条）：那条 `user` 多一格 `shell`，还没跑完的是 `{"running": true}`，
//! 跑完了照 `shell.ran` 填上退出码或信号、输出、截掉几个字、用时、异常结束的原因；翻页和接着推的画得一样；别的话没有这一格。

use miyu_kernel::event::ShellReason;
use miyu_kernel::testkit::{Line, ShellPlay};
use serde_json::json;

use crate::support::*;

#[test]
fn a_command_carries_its_output_under_the_line_she_heard() {
    let mut stage = stage();
    stage.shell_plays([ShellPlay::Exits(3, "nope\n".to_string())]);
    stage.model([Line::says("文件不在。")]);
    stage.shell("cat nope");
    let entries = same(&stage);
    let users = of_kind(&entries, "user");
    assert_eq!(users.len(), 1, "{entries:#?}");
    let user = json(users[0]);
    assert_eq!(user["text"], "cat nope");
    assert_eq!(
        user["shell"],
        json!({"exit": 3, "text": "nope\n", "omitted": 0, "duration_ms": 5}),
        "{user}"
    );
}

#[test]
fn a_running_command_says_so_until_it_is_done() {
    let mut stage = stage();
    stage.shell_plays([ShellPlay::Holds]);
    stage.model([Line::says("跑了一半。")]);
    stage.shell("sleep 100");
    let (entries, _) = live(&stage);
    let user = json(of_kind(&entries, "user")[0]);
    assert_eq!(user["shell"], json!({"running": true}), "{user}");
    stage.finish_shell(None, Some(ShellReason::Interrupted), "half\n");
    let entries = same(&stage);
    let user = json(of_kind(&entries, "user")[0]);
    assert_eq!(
        user["shell"],
        json!({"text": "half\n", "omitted": 0, "reason": "interrupted", "duration_ms": 5}),
        "{user}"
    );
}

#[test]
fn a_plain_message_has_no_shell() {
    let mut stage = stage();
    stage.model([Line::says("嗯。")]);
    stage.say("hi");
    let entries = same(&stage);
    let user = json(of_kind(&entries, "user")[0]);
    assert!(user.get("shell").is_none(), "{user}");
}
