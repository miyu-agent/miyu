//! 记谁开着哪个会话（施工 HV-1）：只有「有没有头开着」变了的才交回来，两个头开着同一个、一个走了的不算变。认领发后台通知
//! 同样只在变了时推（施工 TR-1）。

use miyu_kernel::id::SessionId;

use super::Viewing;

fn id(n: u8) -> SessionId {
    SessionId::parse(&format!("01a19a00-0000-7000-8000-0000000000{n:02}")).expect("合写法")
}

#[test]
fn only_flips_of_viewed_are_reported() {
    let viewing = Viewing::default();
    assert_eq!(viewing.set(1, Some(id(1))), [id(1)], "第一个头开着它");
    assert_eq!(viewing.set(2, Some(id(1))), [], "第二个头也开着，没变");
    assert_eq!(viewing.set(1, Some(id(1))), [], "同一个再说一遍，没变");
    assert_eq!(
        viewing.set(1, Some(id(2))),
        [id(2)],
        "第一个头换走，1 还有人开着"
    );
    assert_eq!(viewing.set(2, None), [id(1)], "最后一个开着 1 的走了");
    assert_eq!(viewing.ids().into_iter().collect::<Vec<_>>(), [id(2)]);
    assert_eq!(
        viewing.set(1, Some(id(3))),
        [id(2), id(3)],
        "换过去：2 没人开了，3 有人开了"
    );
    assert_eq!(viewing.set(1, None), [id(3)]);
    assert!(viewing.ids().is_empty());
}

/// 认领发后台通知（施工 TR-1）：只在「有没有人认领」变了时推，几个认领的走了一个不推。
#[test]
fn a_claim_is_pushed_only_when_it_flips() {
    let viewing = Viewing::default();
    let notices = crate::schedules::notices::Notices::default();
    let mut pushes = notices.subscribe();
    let claimed = |line: String| -> serde_json::Value {
        let line: serde_json::Value = serde_json::from_str(&line).expect("是 JSON");
        assert_eq!(line["method"], "notices.claimed", "{line}");
        line["params"]["claimed"].clone()
    };
    assert!(!viewing.claimed());
    viewing.count(true, &notices);
    viewing.count(true, &notices);
    assert!(viewing.claimed());
    assert_eq!(
        claimed(pushes.try_recv().expect("推了")),
        serde_json::json!(true)
    );
    assert!(pushes.try_recv().is_err(), "第二个来了不推");
    viewing.count(false, &notices);
    assert!(viewing.claimed(), "还有一个");
    assert!(pushes.try_recv().is_err(), "走了一个不推");
    viewing.count(false, &notices);
    assert!(!viewing.claimed());
    assert_eq!(
        claimed(pushes.try_recv().expect("推了")),
        serde_json::json!(false)
    );
}
