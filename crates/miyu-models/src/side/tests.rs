//! 旁路请求登记（施工 Q-2）：档位对上三个固定池；池是空的落到会话的模型，有成员的落到池。扩展清单的档位照池名认（施工 Q-3）。

use super::*;
use crate::test_support::resolved as config;

#[test]
fn tiers_name_the_fixed_pools_and_shipped_sides_are_sorted() {
    assert_eq!(
        [Tier::Lite, Tier::Normal, Tier::Flagship].map(Tier::pool),
        pools::FIXED
    );
    let ids: Vec<&str> = SHIPPED.iter().map(|side| side.id).collect();
    let mut sorted = ids.clone();
    sorted.sort_unstable();
    assert_eq!(ids, sorted);
    assert_eq!((TITLE.tier, ORGANIZE.tier), (Tier::Lite, Tier::Normal));
    assert_eq!(reference(&TITLE), "@lite");
}

#[test]
fn an_empty_pool_resolves_to_the_session_model() {
    let empty = config("").values();
    assert_eq!(resolved(&empty, &TITLE), None, "池是空的，用会话的模型");
    let filled =
        config("[providers.a]\nlocal = false\n\n[pools.lite]\nmodels = [\"a/x\"]\n").values();
    assert_eq!(resolved(&filled, &TITLE), Some("lite"));
    assert_eq!(resolved(&filled, &ORGANIZE), None, "normal 还是空的");
}

/// 扩展清单 `[[side]]` 认的档位（`miyu-config` 那边一份，它在下层、用不上这里的）就是三个固定池，照池名认得回来（施工 Q-3）。
#[test]
fn manifest_tiers_are_the_fixed_pools() {
    assert_eq!(miyu_config::package::SIDE_TIERS, pools::FIXED);
    for name in pools::FIXED {
        assert_eq!(Tier::parse(name).map(Tier::pool), Some(name));
    }
    assert_eq!(Tier::parse("standard"), None);
    let id = String::from("bot/judge");
    let side = Side {
        id: &id,
        tier: Tier::Lite,
    };
    assert_eq!(reference(&side), "@lite", "扩展的编号借清单里的字");
}
