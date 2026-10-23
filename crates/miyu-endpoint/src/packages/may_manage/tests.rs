//! 成员不能改机器上的软件包（施工 Y-5 下下）：管理员能；不是管理员的 `not_permitted`。

use super::*;

#[test]
fn only_admins_manage_packages() {
    assert!(may_manage(true).is_ok());
    assert_eq!(
        may_manage(false).map_err(|refusal| refusal.reason),
        Err("not_permitted")
    );
}
