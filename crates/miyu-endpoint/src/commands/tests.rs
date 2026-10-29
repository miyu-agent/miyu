//! 认命令、判谁能用（施工 O-6）：协议上走不到的几种在这里守，例如群里的主人（对应表里有、又不是会话属主的外部身份，要等
//! 有第二个账号才造得出来）。

use miyu_kernel::id::{ExternalId, VenueId};
use miyu_kernel::origin::{By, External, Person, Role};

use miyu_store::human::Human;
use miyu_store::resources::ResourceRoot;

use super::why::{missing_arguments, no_reply};
use super::{builtin, may_run, sentence, split};
use crate::refusal::Refusal;

fn external(owner: bool, role: Option<Role>) -> By {
    By::External(External {
        venue: VenueId::parse("qq:group:1").expect("合写法"),
        id: ExternalId::parse("qq:10001").expect("合写法"),
        owner,
        role,
    })
}

#[test]
fn the_owner_and_managers_may_run_and_others_may_not() {
    assert!(may_run(&By::Person(Person::default())));
    assert!(may_run(&external(true, Some(Role::Member))), "群里的主人");
    assert!(may_run(&external(false, Some(Role::Manager))));
    assert!(!may_run(&external(false, Some(Role::Member))));
    assert!(!may_run(&external(false, None)));
    assert!(!may_run(&By::Kernel));
}

/// 照内置的登记表认 `text`：交回正名和后面跟的字，认不出的交回拒绝的原因。
fn parsed(text: &str) -> Result<(&'static str, String), &'static str> {
    let commands = builtin();
    let (name, rest) = split(text).map_err(|refusal| refusal.reason)?;
    let slash = commands.find(name).ok_or(Refusal::UNKNOWN_COMMAND.reason)?;
    Ok((slash.name(), rest.to_string()))
}

#[test]
fn names_and_aliases() {
    let ok = |name: &'static str, rest: &str| Ok((name, rest.to_string()));
    assert_eq!(parsed("/clear"), ok("clear", ""));
    assert_eq!(parsed("/reset"), ok("clear", ""));
    assert_eq!(parsed("\t/stop now"), ok("stop", "now"));
    assert_eq!(parsed("/stop\nnow"), ok("stop", "now"));
    assert_eq!(parsed("/workspace  ~/a b \n"), ok("workspace", "~/a b"));
    assert_eq!(
        parsed("/remember  用户 喜欢猫 "),
        ok("remember", "用户 喜欢猫")
    );
    assert_eq!(parsed("/remember"), ok("remember", ""));
    assert_eq!(parsed("/dream"), ok("dream", ""));
    assert_eq!(parsed("/Stop"), Err("unknown_command"));
    assert_eq!(parsed("stop"), Err("bad_params"));
}

#[test]
fn only_dream_is_answered_later() {
    let commands = builtin();
    let later: Vec<&str> = commands
        .all()
        .filter(|slash| slash.later())
        .map(|slash| slash.name())
        .collect();
    assert_eq!(later, ["dream"]);
}

/// 核心自己说的为什么（施工 X-39 补）：三种语言都有，几个参数名照这门语言连起来。
#[test]
fn reasons_the_core_gives_itself_are_in_every_language() {
    let root = ResourceRoot::at(crate::test_support::resources());
    for language in ["zh", "en", "ja"] {
        let human = Human::load(&root, language).expect("读得出");
        let missing = sentence(
            &human,
            &missing_arguments(vec!["code".to_string(), "focus".to_string()]),
        );
        assert!(
            missing.contains("code") && missing.contains("focus") && !missing.contains('{'),
            "{language}：{missing}"
        );
        let no_reply = sentence(&human, &no_reply("weather"));
        assert!(no_reply.contains("/weather"), "{language}：{no_reply}");
    }
}
