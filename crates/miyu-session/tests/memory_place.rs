//! 记下在哪个群（施工 E-4 上，`docs/construction/E-4上-记下在哪个群.md`）：桥每条场所消息带 `venue.place`（这个群此刻叫
//! 什么）。记忆记下时存进 `place`：抽取记的照出处那一轮那时的群名，群改了名以后新记的是新名、以前记的不变；她当场记的照这个
//! 会话最近听到的，载入以后照日志接上；改一条的这个会话没听到过群名就照旧的。群名只给记忆：请求一个字节都不因它变。
//! 整理记忆的模型是一台假服务器；名字、群号、群名都是编的。

use miyu_http::testkit::Server;
use miyu_kernel::block::{Block, Text};
use miyu_kernel::event::{Body, VenueMessage};
use miyu_kernel::id::{ExternalId, VenueId};
use miyu_kernel::origin::{By, External, Person, Role};
use miyu_kernel::request::{Message, Request};
use miyu_kernel::session::Command;
use miyu_recall::{MemoryEvent, Saved};
use miyu_session::testkit::{Play, Script};
use miyu_session::{Filter, Keeper, Listener, Stamp};
use miyu_store::recall::Room;
use miyu_tool::{Catalog, Remember};

use crate::support::extracting::*;
use crate::support::meaning::{catalog, persona};
use crate::support::*;

fn group() -> VenueId {
    VenueId::parse("qq:group:7").expect("合写法")
}

/// 群里的小林（`qq:20017`）。
fn member() -> By {
    By::External(External {
        venue: group(),
        id: ExternalId::parse("qq:20017").expect("合写法"),
        owner: false,
        role: Some(Role::Member),
    })
}

/// 小林在群里说 `words`（平台的编号 `msg`）、@ 了她，这时群叫 `place`（没有的不带）；`ambient` 的是旁听，不开回合。
fn heard(words: &str, msg: &str, place: Option<&str>, ambient: bool) -> Command {
    Command::Send {
        blocks: vec![Block::Text(Text {
            text: words.to_string(),
        })],
        urgent: false,
        venue: Some(VenueMessage {
            msg: msg.to_string(),
            name: Some("小林".to_string()),
            place: place.map(str::to_string),
            mentions_me: !ambient,
            ambient,
            ..VenueMessage::default()
        }),
    }
}

/// `by` 说一句，等这一轮说完。
async fn one_turn(handle: &miyu_session::Handle, command: &str, said: Command) {
    let mut pushes = watch(handle).await;
    within("回应", handle.command(id(command), member(), said))
        .await
        .expect("会话在跑");
    until_turn_ends(&mut pushes).await;
}

/// 照 `keeper` 列出正文和在哪个群，照正文排。
fn listed(keeper: &Keeper) -> Vec<(String, Option<String>)> {
    let mut found: Vec<(String, Option<String>)> = keeper
        .list(&Filter {
            limit: 50,
            ..Filter::default()
        })
        .expect("列得出")
        .into_iter()
        .map(|entry| (entry.text, entry.place))
        .collect();
    found.sort();
    found
}

/// 抽取记的照出处那一轮那时的群名：两轮一起抽，中间群改了名，头一轮记下的是旧名、后一轮的是新名（不照这一段最后的群名
/// 一起写）。
#[tokio::test]
async fn extracted_memories_keep_the_group_name_of_their_turn() {
    let server = Server::start(vec![every_turn("小林在学日语")]).await;
    let mut home = Home::new();
    organizer(&mut home, &server, "extract_turns = 2\n");
    let script = Script::new([Play::Says("加油。"), Play::Says("厉害。")]);
    let lines = Lines {
        venue: group(),
        group: true,
        ..Lines::default()
    };
    let handle = home
        .create_full(&script, &catalog(&home), Opening::default(), lines)
        .await;
    one_turn(
        &handle,
        "cmd-1",
        heard("我在学日语", "8801", Some("周末摄影群"), false),
    )
    .await;
    one_turn(
        &handle,
        "cmd-2",
        heard("我报了日语考试", "8802", Some("摄影爱好者"), false),
    )
    .await;
    requested(&server, 1).await;
    let owner = Keeper::new(
        &home.memory,
        Room::persona("engineer"),
        Listener::Owner(By::Person(Person::default())),
    );
    assert_eq!(
        wait_for(&owner, 2).await,
        [
            ("小林在学日语".to_string(), Some("周末摄影群".to_string())),
            ("小林在学日语".to_string(), Some("摄影爱好者".to_string())),
        ],
        "各照各那一轮的"
    );
    stop(&handle).await;
}

/// 等这一间列得出 `count` 条（最多 10 秒）。
async fn wait_for(keeper: &Keeper, count: usize) -> Vec<(String, Option<String>)> {
    let mut found = Vec::new();
    for _ in 0..200 {
        found = listed(keeper);
        if found.len() >= count {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    found
}

/// 她当场记的照这个会话最近听到的群名（旁听的那一句也算）；载入以后照日志接上，人这一句没带也照样写；没听到过群名的会话
/// 不写。
#[tokio::test]
async fn remembering_keeps_the_latest_group_name_heard_and_it_survives_a_load() {
    let home = Home::new();
    let script = Script::new([
        Play::calls(&[("remember", r#"{"class":"user","text":"群里周六去拍樱花"}"#)]),
        Play::Says("记住了。"),
        Play::calls(&[("remember", r#"{"class":"user","text":"群里周日聚餐"}"#)]),
        Play::Says("记住了。"),
    ]);
    let lines = Lines {
        venue: group(),
        group: true,
        ..Lines::default()
    };
    let handle = home
        .create_full(&script, &catalog(&home), Opening::default(), lines)
        .await;
    let session = handle.id().clone();
    within(
        "回应",
        handle.command(
            id("cmd-1"),
            member(),
            heard("早", "8800", Some("周末摄影群"), true),
        ),
    )
    .await
    .expect("会话在跑");
    one_turn(
        &handle,
        "cmd-2",
        heard("周六去拍樱花，记一下", "8801", Some("摄影爱好者"), false),
    )
    .await;
    stop(&handle).await;
    let keeper = Keeper::new(&home.memory, persona(), Listener::Owner(alice()));
    assert_eq!(
        listed(&keeper),
        [(
            "群里周六去拍樱花".to_string(),
            Some("摄影爱好者".to_string())
        )],
        "照最近听到的那个"
    );

    let handle = home.load_with(&session, &script, &catalog(&home)).await;
    one_turn(
        &handle,
        "cmd-3",
        heard("周日聚餐也记一下", "8802", None, false),
    )
    .await;
    stop(&handle).await;
    assert_eq!(
        listed(&keeper),
        [
            (
                "群里周六去拍樱花".to_string(),
                Some("摄影爱好者".to_string())
            ),
            ("群里周日聚餐".to_string(), Some("摄影爱好者".to_string())),
        ],
        "载入以后照日志接上"
    );

    // 没听到过群名的会话（私聊、本机）不写。
    let script = Script::new([
        Play::calls(&[("remember", r#"{"class":"user","text":"主人喜欢猫"}"#)]),
        Play::Says("记住了。"),
    ]);
    let handle = home.create_with(&script, &catalog(&home)).await;
    let mut pushes = watch(&handle).await;
    within(
        "回应",
        handle.command(id("cmd-1"), alice(), say("我喜欢猫，记一下")),
    )
    .await
    .expect("会话在跑");
    until_turn_ends(&mut pushes).await;
    stop(&handle).await;
    assert!(
        listed(&keeper).contains(&("主人喜欢猫".to_string(), None)),
        "{:?}",
        listed(&keeper)
    );
}

/// 改一条：这次没听到过群名的（人经协议改的）照旧那条的群名。
#[tokio::test]
async fn a_revision_without_a_group_name_keeps_the_old_one() {
    let home = Home::new();
    let (log, _) = home.logs.open(&persona()).expect("开得了");
    let old = log
        .append(
            now(),
            alice(),
            None,
            &MemoryEvent::Saved(Saved {
                class: "user".into(),
                text: "群里周六去拍樱花".into(),
                sources: Vec::new(),
                audience: vec![alice()],
                replaces: None,
                about: None,
                venue: None,
                keywords: None,
                who: None,
                people: Vec::new(),
                place: Some("周末摄影群".to_string()),
            }),
        )
        .expect("记得下")
        .id;
    let keeper = Keeper::new(&home.memory, persona(), Listener::Owner(alice()));
    let stamp = Stamp {
        at: now(),
        by: alice(),
        cause: Some(id("p1")),
    };
    keeper
        .save(stamp, remember("群里周日去拍樱花", Some(old)), Vec::new())
        .expect("改得了");
    assert_eq!(
        listed(&keeper),
        [(
            "群里周日去拍樱花".to_string(),
            Some("周末摄影群".to_string())
        )]
    );
}

fn remember(text: &str, replaces: Option<miyu_recall::MemoryId>) -> Remember {
    Remember {
        class: "user".to_string(),
        text: text.to_string(),
        replaces,
        keywords: None,
        who: None,
    }
}

/// 请求里的全部，时刻（`[HH:MM]`）换成一样的：两个会话前后差几毫秒，钟可能刚好跨一分钟。
fn shape(request: &Request) -> String {
    let text = format!("{request:?}");
    let mut out = String::new();
    let mut rest = text.as_str();
    while let Some(at) = rest.find('[') {
        out.push_str(&rest[..at]);
        let tail = &rest[at..];
        let clock = tail.len() >= 7
            && tail.as_bytes()[6] == b']'
            && tail[1..6].chars().enumerate().all(|(n, c)| match n {
                2 => c == ':',
                _ => c.is_ascii_digit(),
            });
        if clock {
            out.push_str("[hh:mm]");
            rest = &tail[7..];
        } else {
            out.push('[');
            rest = &tail[1..];
        }
    }
    out.push_str(rest);
    out
}

/// 群名只给记忆：同样两句话，带群名和不带群名的两个群会话，发给模型的请求除了钟一个字节都不差，请求里也找不到群名。
#[tokio::test]
async fn the_group_name_does_not_change_the_request() {
    let home = Home::new();
    let mut shapes = Vec::new();
    for (n, place) in [(1, Some("周末摄影群")), (2, None)] {
        let script = Script::new([Play::Says("我来。")]);
        let handle = home
            .create_full(
                &script,
                &Catalog::default(),
                Opening::default(),
                Lines {
                    venue: group(),
                    group: true,
                    ..Lines::default()
                },
            )
            .await;
        within(
            "回应",
            handle.command(id("cmd-1"), member(), heard("早", "8800", place, true)),
        )
        .await
        .expect("会话在跑");
        one_turn(&handle, "cmd-2", heard("今天谁值班", "8801", place, false)).await;
        stop(&handle).await;
        let log = home.log(handle.id());
        assert!(
            log.iter()
                .any(|event| matches!(&event.body, Body::MessageUser(message)
                if message.venue.as_ref().and_then(|venue| venue.place.as_deref()) == place)),
            "{n}: 日志里照写"
        );
        let requests = script.requests();
        let (_, request) = requests.first().expect("发了一次");
        assert!(
            !format!("{request:?}").contains("周末摄影群"),
            "{n}: 请求里没有群名"
        );
        assert!(
            request
                .messages
                .iter()
                .any(|message| matches!(message, Message::User { .. })),
            "{n}"
        );
        shapes.push(shape(request).replace(&handle.id().to_string(), "<session>"));
    }
    assert_eq!(shapes[0], shapes[1], "带不带群名请求一样");
}
