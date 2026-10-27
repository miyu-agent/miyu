//! 以前的对话打不开的不给编号（施工 E-6，`docs/blueprint/memory.md`「工具」）：`memory_search` 照端口交回的打不打得开、在哪个
//! 场所写那一行。端口是假的：真的在 `miyu-session` 的 `memory_past_opens.rs` 里测。

use std::sync::Arc;

use miyu_kernel::id::{Seq, SessionId, TurnId, VenueId};
use miyu_kernel::time::UtcOffset;
use miyu_tool::{
    Call, Done, FoundTurn, Listing, MemoryPort, Opening, Progress, Searched, SessionsPort, Stop,
};

use crate::tools::{Fake, at, call, text, tools};

/// 本机的主会话里调（施工 E-6）：有列会话的端口，`history` 读得了别的会话。
pub(crate) async fn call_reading(port: Arc<Fake>, name: &str, args: serde_json::Value) -> Done {
    let tool = tools()
        .into_iter()
        .find(|tool| tool.spec().name == name)
        .expect("有这件");
    let call = Call {
        args: args.to_string(),
        cwd: String::new(),
        offset: UtcOffset::from_minutes(0).expect("零时区"),
        memory: Some(port as Arc<dyn MemoryPort>),
        sessions: Some(Arc::new(Lister(
            SessionId::parse("0192f3a0-9999-7abc-8def-999999999999").expect("合写法"),
        ))),
        ..miyu_tool::testkit::call("{}")
    };
    tool.run(call, Progress::new(|_| {})).await
}

/// 列会话的端口：这里只要它在（她读得了别的会话），列、开都不会走到。
struct Lister(SessionId);

impl SessionsPort for Lister {
    fn this(&self) -> &SessionId {
        &self.0
    }

    fn list<'a>(&'a self, _stop: &'a Stop) -> Listing<'a> {
        Box::pin(async { Ok(Vec::new()) })
    }

    fn open<'a>(&'a self, _session: &'a SessionId) -> Opening<'a> {
        Box::pin(async { Err("not reached in this test".to_string()) })
    }
}

/// 以前的对话里她打不开的会话（施工 E-6）：不给编号、写在哪聊的。群会话写是群，私聊写私聊，本机别人的、分叉出来的支线什么都
/// 不加；打得开的照旧给编号，可她在场所会话、子会话里（没有列会话的端口）搜的一律不给。
#[tokio::test]
async fn past_conversations_she_cannot_open_say_where_instead_of_an_id() {
    let session = SessionId::parse("0192f3a0-1111-7abc-8def-001122334455").expect("合写法");
    let turn = |text: &str, venue: Option<&str>, opens: bool| FoundTurn {
        session: session.clone(),
        turn: TurnId::new(Seq::new(8).expect("从 1 起")),
        at: at(6),
        text: text.into(),
        venue: venue.map(|venue| VenueId::parse(venue).expect("合写法")),
        opens,
    };
    let found = Searched {
        memories: Vec::new(),
        turns: vec![
            turn("本机的", None, true),
            turn("群里的", Some("qq:group:900001"), false),
            turn("私聊的", Some("qq:private:10086"), false),
            turn("支线的", None, false),
            turn("主人自己的私聊", Some("qq:private:20002"), true),
        ],
        cards: Vec::new(),
    };
    let port = Arc::new(Fake {
        found: found.clone(),
        ..Fake::default()
    });
    let done = call_reading(port, "memory_search", serde_json::json!({"query":"显卡"})).await;
    assert_eq!(
        text(&done),
        "2026-10-06 session 22334455: 本机的\n\
         2026-10-06 in a QQ group: 群里的\n\
         2026-10-06 in a QQ private chat: 私聊的\n\
         2026-10-06: 支线的\n\
         2026-10-06 session 22334455: 主人自己的私聊"
    );
    // 给头看的那一份照旧写整个编号：人打得开。
    assert_eq!(
        done.detail.as_ref().expect("有")["turns"][1]["session"],
        serde_json::json!("0192f3a0-1111-7abc-8def-001122334455")
    );
    // 场所会话、子会话里没有列会话的端口：打得开的也不给。
    let port = Arc::new(Fake {
        found,
        ..Fake::default()
    });
    let done = call(
        Some(port),
        "memory_search",
        serde_json::json!({"query":"显卡"}),
    )
    .await;
    assert_eq!(
        text(&done),
        "2026-10-06: 本机的\n\
         2026-10-06 in a QQ group: 群里的\n\
         2026-10-06 in a QQ private chat: 私聊的\n\
         2026-10-06: 支线的\n\
         2026-10-06 in a QQ private chat: 主人自己的私聊"
    );
}
