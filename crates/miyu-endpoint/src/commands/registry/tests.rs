//! 登记表（施工 O-6 三补）：内置的六条照名字排；名字、别名撞了登记时就拦；别名和别的命令的别名不一样的不算撞。

use std::sync::Arc;

use super::*;
use crate::commands::builtin;

/// 只有名字、别名的一条，测撞名用。
struct Named(&'static str, &'static [&'static str]);

impl Slash for Named {
    fn name(&self) -> &'static str {
        self.0
    }

    fn aliases(&self) -> &'static [&'static str] {
        self.1
    }

    fn summary(&self) -> &'static str {
        "commands/summary/clear"
    }

    fn run<'a>(&'a self, _call: Call<'a>) -> Running<'a> {
        Box::pin(async { Err(Refusal::INTERNAL) })
    }
}

#[test]
fn the_built_in_commands_are_registered_in_name_order() {
    let commands = builtin();
    let names: Vec<&str> = commands.all().map(|slash| slash.name()).collect();
    assert_eq!(
        names,
        [
            "clear",
            "clear-memory",
            "dream",
            "goal",
            "remember",
            "stop",
            "workspace"
        ]
    );
    assert_eq!(
        commands.find("reset").map(|slash| slash.name()),
        Some("clear")
    );
    assert!(commands.find("help").is_none());
}

#[test]
fn a_different_alias_is_not_a_clash() {
    // 记忆的 `/clear-memory`（别名 `/reset-memory`，施工 R-3 五补）：和 `/clear`、`/reset` 是不同的词。
    let commands = builtin();
    assert_eq!(
        commands.find("reset-memory").map(|slash| slash.name()),
        Some("clear-memory")
    );
    assert_eq!(
        commands.find("reset").map(|slash| slash.name()),
        Some("clear")
    );
}

#[test]
#[should_panic(expected = "斜杠命令 /reset 撞名")]
fn a_name_that_is_already_an_alias_is_refused() {
    let mut commands = builtin();
    commands.register(Arc::new(Named("reset", &[])));
}

#[test]
#[should_panic(expected = "斜杠命令 /stop 撞名")]
fn an_alias_that_is_already_a_name_is_refused() {
    let mut commands = builtin();
    commands.register(Arc::new(Named("halt", &["stop"])));
}
