//! 提供者的工具（施工 O-2 下）：`tool.call` 带是谁要的、是不是主人；超时交回那一句并发 `tool.cancel`；叫它停发 `tool.cancel`、
//! 照样等它回；掐掉（future 被丢掉）发 `tool.cancel`；回应先到的不发。

use miyu_kernel::block::{Block, Text};
use miyu_kernel::id::{AccountId, CallId, ExternalId, Seq, SessionId, VenueId};
use miyu_kernel::origin::{By, External, Person};
use miyu_tool::{CallIds, Done, Progress};
use tokio::sync::mpsc;

use super::*;
use crate::wire::Response;

const SESSION: &str = "01900000-0000-7000-8000-000000000001";

/// 一件提供者的工具，等 `timeout`；它的包由一个假连接提供，交回往外写的那一头收的。
fn remote(timeout: Duration) -> (RemoteTool, Peer, mpsc::Receiver<String>) {
    remote_with(timeout, stores().0)
}

/// 同 [`remote`]，扩展传上来的 blob 在 `stores`。
fn remote_with(timeout: Duration, stores: Stores) -> (RemoteTool, Peer, mpsc::Receiver<String>) {
    let provided = Arc::new(Provided::default());
    let (out, lines) = mpsc::channel(8);
    let peer = Peer::new(out);
    provided.register(Instance::system("onebot"), peer.clone());
    let mut checked = checked(tool("read", json!(["local"]))).expect("合写法");
    checked.timeout = timeout;
    let remote = RemoteTool::new(checked, "onebot", provided, texts(), stores);
    (remote, peer, lines)
}

/// 一次调用：会话（属主是管理员）、编号 7 的第 1 件，`asked` 要的。
fn call_by(asked: Option<By>) -> miyu_tool::Call {
    call_of("admin", asked)
}

/// 同 [`call_by`]，会话的属主是 `owner`。
fn call_of(owner: &str, asked: Option<By>) -> miyu_tool::Call {
    let mut call = miyu_tool::testkit::call(r#"{"to":"群"}"#);
    call.ids = Some(CallIds {
        session: SessionId::parse(SESSION).expect("合写法"),
        owner: AccountId::parse(owner).expect("合写法"),
        call: CallId::new(Seq::new(7).expect("合写法"), 1).expect("合写法"),
        asked,
        preset: None,
    });
    call
}

/// 读下一行，认成 JSON。
async fn next(lines: &mut mpsc::Receiver<String>) -> Value {
    serde_json::from_str(&lines.recv().await.expect("有一行")).expect("是 JSON")
}

fn text(done: &miyu_tool::Done) -> String {
    match done.blocks.as_slice() {
        [Block::Text(Text { text })] => text.clone(),
        other => panic!("{other:?}"),
    }
}

/// 群里的 `n` 号人；`account` 是主人对应表里对着的账号。
fn member(n: u32, account: Option<&str>) -> By {
    By::External(External {
        venue: VenueId::parse("qq:group:1").unwrap(),
        id: ExternalId::parse(&format!("qq:{n}")).unwrap(),
        account: account.map(|name| AccountId::parse(name).unwrap()),
        role: None,
    })
}

/// 调一次，交回发出去的 `tool.call` 的参数；回一句 `done`，交回结果。
async fn sent_for(asked: Option<By>) -> (Value, String) {
    let (remote, peer, mut lines) = remote(Duration::from_secs(60));
    let running =
        tokio::spawn(async move { remote.run(call_by(asked), Progress::new(|_| {})).await });
    let sent = next(&mut lines).await;
    peer.answer(Response {
        id: sent["id"].as_str().unwrap().to_string(),
        outcome: Ok(json!({"blocks": [{"type": "text", "text": "done"}]})),
    });
    let done = running.await.unwrap();
    assert!(lines.try_recv().is_err(), "回应先到的不发 tool.cancel");
    (sent, text(&done))
}

#[tokio::test]
async fn a_call_carries_who_asked_and_whether_it_is_the_owner() {
    let (sent, answer) = sent_for(Some(member(1, Some("alice")))).await;
    assert_eq!(answer, "done");
    assert_eq!(sent["method"], "tool.call");
    assert_eq!(sent["params"]["by"], json!(member(1, Some("alice"))));
    assert_eq!(sent["params"]["owner"], json!(true), "对应表里有的是主人");
    assert_eq!(
        sent["params"]["call_id"],
        json!(call_by(None).ids.unwrap().call.to_string())
    );
    let (sent, _) = sent_for(Some(By::Person(Person::new(
        AccountId::parse("alice").unwrap(),
    ))))
    .await;
    assert_eq!(sent["params"]["owner"], json!(true), "本机的人是主人");
    let (sent, _) = sent_for(Some(member(2, None))).await;
    assert_eq!(sent["params"]["owner"], json!(false), "不在对应表里的不是");
    let (sent, _) = sent_for(None).await;
    assert_eq!(sent["params"]["owner"], json!(false), "没有触发的不是");
    assert!(sent["params"].get("by").is_none(), "没有触发的不写 by");
}

#[tokio::test]
async fn a_call_not_answered_in_time_is_timed_out_and_cancelled() {
    let (remote, _peer, mut lines) = remote(Duration::from_millis(50));
    let done = remote.run(call_by(None), Progress::new(|_| {})).await;
    assert!(done.error);
    assert_eq!(
        text(&done),
        "The tool \"send_group\" did not answer within 1 seconds.\n"
    );
    assert_eq!(
        done.human,
        Some(
            miyu_kernel::event::Said::new("core/tool-results/timed-out")
                .with("name", "send_group")
                .with("seconds", "1")
        )
    );
    assert_eq!(next(&mut lines).await["method"], "tool.call");
    let cancel = next(&mut lines).await;
    assert_eq!(
        cancel,
        json!({"jsonrpc": "2.0", "method": "tool.cancel",
               "params": {"session": SESSION, "call_id": call_by(None).ids.unwrap().call.to_string()}})
    );
    assert!(lines.try_recv().is_err(), "只发一次");
}

#[tokio::test]
async fn a_call_told_to_stop_is_cancelled_and_still_answered() {
    let (remote, peer, mut lines) = remote(Duration::from_secs(60));
    let call = call_by(None);
    let stop = call.stop.clone();
    let running = tokio::spawn(async move { remote.run(call, Progress::new(|_| {})).await });
    let sent = next(&mut lines).await;
    stop.raise();
    assert_eq!(next(&mut lines).await["method"], "tool.cancel");
    peer.answer(Response {
        id: sent["id"].as_str().unwrap().to_string(),
        outcome: Ok(
            json!({"blocks": [{"type": "text", "text": "stopped halfway"}], "error": true}),
        ),
    });
    let done = running.await.unwrap();
    assert_eq!(text(&done), "stopped halfway", "照样等它回");
    assert!(lines.try_recv().is_err(), "只发一次");
}

#[tokio::test]
async fn a_call_dropped_while_waiting_is_cancelled() {
    let (remote, _peer, mut lines) = remote(Duration::from_secs(60));
    let running =
        tokio::spawn(async move { remote.run(call_by(None), Progress::new(|_| {})).await });
    assert_eq!(next(&mut lines).await["method"], "tool.call");
    running.abort();
    assert!(running.await.is_err());
    assert_eq!(next(&mut lines).await["method"], "tool.cancel");
}

/// 叫它停发过了，再超时没回：不再发第二次。
#[tokio::test]
async fn a_call_told_to_stop_and_then_timed_out_is_cancelled_once() {
    let (remote, _peer, mut lines) = remote(Duration::from_millis(400));
    let call = call_by(None);
    let stop = call.stop.clone();
    let running = tokio::spawn(async move { remote.run(call, Progress::new(|_| {})).await });
    assert_eq!(next(&mut lines).await["method"], "tool.call");
    stop.raise();
    assert_eq!(next(&mut lines).await["method"], "tool.cancel");
    let done = running.await.unwrap();
    assert!(text(&done).contains("did not answer"), "{}", text(&done));
    assert!(lines.try_recv().is_err(), "只发一次");
}

/// 一张 1×1 的图片块，引用 `blob`。
fn image(blob: miyu_kernel::id::ContentHash) -> Block {
    Block::Image(miyu_kernel::block::Image {
        blob,
        name: None,
        media_type: miyu_kernel::id::MediaType::parse("image/png").expect("合写法"),
        width: 1,
        height: 1,
        path: None,
    })
}

/// 属主是 `owner` 的会话调一次，回 `blocks`，交回结果。
async fn answered_with(stores: Stores, owner: &str, blocks: Vec<Block>) -> miyu_tool::Done {
    let (remote, peer, mut lines) = remote_with(Duration::from_secs(60), stores);
    let call = call_of(owner, None);
    let running = tokio::spawn(async move { remote.run(call, Progress::new(|_| {})).await });
    let sent = next(&mut lines).await;
    peer.answer(Response {
        id: sent["id"].as_str().unwrap().to_string(),
        outcome: Ok(json!({ "blocks": blocks })),
    });
    running.await.unwrap()
}

/// 测完删掉临时的数据根；删不掉就留在临时目录里，不影响测试。
fn forget(dir: std::path::PathBuf) {
    if std::fs::remove_dir_all(dir).is_err() {
        // 留着。
    }
}

/// 扩展传进管理员名下的图（施工 O-2 三补）：交回给系统账号的会话时拷一份进它名下，块原样不动。
#[tokio::test]
async fn an_answered_image_is_handed_over_to_the_sessions_owner() {
    let (place, dir) = stores();
    let admin = miyu_store::blob::Blobs::new(place.root.blobs(&place.provider));
    let blob = admin.put(b"not really a png").expect("存得进");
    let owner = AccountId::parse("onebot").expect("合写法");
    let theirs = miyu_store::blob::Blobs::new(place.root.blobs(&owner));
    assert!(!theirs.path(&blob).is_file());
    let done = answered_with(place, "onebot", vec![image(blob.clone())]).await;
    assert!(!done.error, "{:?}", done.blocks);
    assert_eq!(done.blocks, [image(blob.clone())]);
    assert!(theirs.path(&blob).is_file(), "拷进了属主名下");
    forget(dir);
}

/// 提供者名下也没有的 blob：照暂时不可用，不把读不到的图交给她；属主就是提供者的也查在不在。
#[tokio::test]
async fn an_image_nobody_uploaded_makes_the_tool_unavailable() {
    let (elsewhere, first) = stores();
    let missing = miyu_store::blob::Blobs::new(elsewhere.root.blobs(&elsewhere.provider))
        .put(b"x")
        .expect("存得进");
    for owner in ["onebot", "admin"] {
        let (place, dir) = stores();
        let done = answered_with(place, owner, vec![image(missing.clone())]).await;
        assert!(done.error, "{owner}");
        assert_eq!(
            text(&done),
            "The tool \"send_group\" is not available right now.\n"
        );
        forget(dir);
    }
    forget(first);
}

/// 拷进会话属主名下的 blob（施工 W-11 补）：图片块、发出去的图、`images` 形状里的图都算；别的形状、读不成的不算。
#[test]
fn the_blobs_a_result_references_include_detail_images() {
    let hash = |n: u8| miyu_kernel::id::ContentHash::of(&[n]);
    let mut done = Done::ok("x");
    done.detail = Some(
        json!({"kind": "images", "images": [{"blob": hash(1).as_str(), "media_type": "image/png"}]}),
    );
    assert_eq!(referenced(&done), [hash(1)]);
    done.detail = Some(json!({"kind": "links", "links": []}));
    assert!(referenced(&done).is_empty());
    done.detail = Some(
        json!({"kind": "images", "images": [{"blob": hash(2).as_str(), "media_type": "text/plain"}]}),
    );
    assert!(referenced(&done).is_empty(), "读不成的已经丢了，不拷");
}

/// 给脚本的结构化结果（施工 X-33）：照收、原样交进 `data`；出错的不交，同 MCP 的 `structuredContent`。
#[tokio::test]
async fn data_is_handed_over_unless_the_call_failed() {
    for (error, wanted) in [(false, Some(json!({"items": [{"n": 1}]}))), (true, None)] {
        let (remote, peer, mut lines) = remote(Duration::from_secs(60));
        let running =
            tokio::spawn(async move { remote.run(call_by(None), Progress::new(|_| {})).await });
        let sent = next(&mut lines).await;
        peer.answer(Response {
            id: sent["id"].as_str().unwrap().to_string(),
            outcome: Ok(json!({
                "blocks": [{"type": "text", "text": "found"}],
                "error": error,
                "data": {"items": [{"n": 1}]},
            })),
        });
        let done = running.await.unwrap();
        assert_eq!(done.error, error);
        assert_eq!(done.data, wanted, "error={error}");
    }
}

/// 有数据根、派它的那一轮在 `venue` 的调用，发出去的参数。
async fn sent_with_root(data: &std::path::Path, venue: Option<&str>) -> Value {
    let mut call = call_of("bob", None);
    call.data_root = Some(data.to_path_buf());
    call.caller = venue.map(|venue| miyu_tool::Caller {
        venue: VenueId::parse(venue).expect("合写法"),
        owner: true,
        model: None,
        log: None,
    });
    let (remote, peer, mut lines) = remote(Duration::from_secs(60));
    let running = tokio::spawn(async move { remote.run(call, Progress::new(|_| {})).await });
    let sent = next(&mut lines).await;
    peer.answer(Response {
        id: sent["id"].as_str().unwrap().to_string(),
        outcome: Ok(json!({"blocks": [{"type": "text", "text": "done"}]})),
    });
    running.await.unwrap();
    sent
}

/// 调扩展的工具时交会话属主的 pictures 位置（施工 Y-11 下）：照数据根的真实位置接上，不建；会话外面的调用（没有数据根）不写。
/// 只给本机的会话（施工 Y-11 下修）：场所会话关在自己的工作区里，图存到属主（桥的系统账号）家里她就碰不到了，几个群还共用
/// 一处；不知道在哪个场所的也不给。
#[tokio::test]
async fn a_call_carries_where_the_owner_s_pictures_go() {
    let data = std::env::temp_dir();
    let sent = sent_with_root(&data, Some("local")).await;
    let real = std::fs::canonicalize(&data).expect("在");
    assert_eq!(
        sent["params"]["pictures"],
        json!(
            real.join("home")
                .join("bob")
                .join("pictures")
                .display()
                .to_string()
        )
    );
    let (without, _) = sent_for(None).await;
    assert!(without["params"].get("pictures").is_none(), "{without}");
}

#[tokio::test]
async fn a_venue_session_s_call_carries_no_pictures() {
    let data = std::env::temp_dir();
    let group = sent_with_root(&data, Some("qq:group:1")).await;
    assert!(group["params"].get("pictures").is_none(), "{group}");
    let unknown = sent_with_root(&data, None).await;
    assert!(unknown["params"].get("pictures").is_none(), "{unknown}");
}
