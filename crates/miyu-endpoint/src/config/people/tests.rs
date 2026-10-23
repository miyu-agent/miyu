//! 照机主那一份造别人的（施工 Y-2 下下中上）：系统配置跟机主的，个人设置读他自己的、不带机主的；`{ env = … }` 不认；系统配置换了
//! 跟着换。

use miyu_config::secret::Reference;
use miyu_kernel::id::AccountId;

use super::super::Environment;
use super::*;
use crate::test_support::temp_root;

fn account(name: &str) -> AccountId {
    AccountId::parse(name).expect("合写法")
}

#[test]
fn a_member_takes_the_system_layer_but_not_the_owners_settings() {
    let (root, _) = temp_root("people");
    let (alice, bob) = (account("alice"), account("bob"));
    std::fs::write(
        root.system().join("config.toml"),
        "[terminal]\nidle_new_after = 7\n",
    )
    .expect("写得进");
    std::fs::create_dir_all(root.account_dir(&alice)).expect("建得了");
    std::fs::write(
        root.account_dir(&alice).join("settings.toml"),
        "[ui]\nlanguage = \"ja\"\n",
    )
    .expect("写得进");
    std::fs::create_dir_all(root.account_dir(&bob)).expect("建得了");
    std::fs::write(
        root.account_dir(&bob).join("settings.toml"),
        "[ui]\nhead = \"web\"\n",
    )
    .expect("写得进");
    let owner = Config::load(
        &root,
        &alice,
        None,
        crate::settings::core_items(),
        Environment::of(&[("SOME_KEY", "secret value")]),
    );
    let member = owner.of_member(&root, &bob);
    let values = member.resolved().values();
    let text = |key: &str| values.get(key).map(|value| format!("{value:?}"));
    assert!(
        text("terminal.idle_new_after").is_some_and(|v| v.contains('7')),
        "系统配置跟机主的"
    );
    assert!(
        !text("ui.language").is_some_and(|v| v.contains("ja")),
        "机主的个人设置不带"
    );
    assert!(
        text("ui.head").is_some_and(|v| v.contains("web")),
        "他自己的个人设置"
    );
    assert_eq!(member.places.account, bob);
    let env = Reference::Env("SOME_KEY".to_string());
    assert!(owner.secret(&env).is_some(), "机主的照核心的环境");
    assert!(member.secret(&env).is_none(), "别人的不认 env");
}
