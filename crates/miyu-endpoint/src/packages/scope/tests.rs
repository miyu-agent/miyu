//! 装卸落在哪一层（施工 U23 下上）：只有系统那一层做得了的，自己那一层的拒 `not_permitted`（施工 SU-1 起没有管理员模式）。

use super::*;

#[test]
fn only_the_system_scope_does_system_things() {
    let own = Scope::Own(AccountId::parse("bob").unwrap());
    assert!(Scope::System.system_only(false).is_ok());
    assert!(Scope::System.system_only(true).is_ok());
    assert_eq!(
        own.system_only(true).map_err(|refusal| refusal.reason),
        Err("not_permitted"),
        "施工 SU-1 起没有管理员模式"
    );
    assert_eq!(
        own.system_only(false).map_err(|refusal| refusal.reason),
        Err("not_permitted")
    );
}
