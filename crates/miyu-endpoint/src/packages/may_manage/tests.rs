//! 只有替主人本人的能改软件包（施工 Y-5 下下）：关在沙盒里的会话 `not_permitted`。

use super::*;

#[test]
fn only_admins_manage_packages() {
    assert!(may_manage(true).is_ok());
    assert_eq!(
        may_manage(false).map_err(|refusal| refusal.reason),
        Err("not_permitted")
    );
}
