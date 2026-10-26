//! 结局怎么写成退出码和印的字（设计 39 第六节第 3、4 条）：照出厂的字。

use std::path::Path;

use miyu_kernel::id::JobId;
use miyu_tool::{Calling, Schema};

use super::*;

/// 出厂的字。
fn texts() -> std::sync::Arc<Texts> {
    let resources = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources");
    Texts::load(&resources).expect("出厂的字读得出")
}

/// 能调 `web_search`、`kb_read` 的假端口；调不到这里。
struct Port;

impl NestedPort for Port {
    fn call(&self, _name: String, _args: String) -> Calling<'_> {
        unreachable!("这里只测写成什么")
    }

    fn adopt(&self, _job: JobId) {}

    fn callable(&self) -> Vec<String> {
        vec!["kb_read".to_string(), "web_search".to_string()]
    }

    fn schema(&self, name: &str) -> Option<Schema> {
        match name {
            "web_search" => Some(Schema {
                description: "Search the web.\n".to_string(),
                parameters: r#"{"type":"object"}"#.to_string(),
                result: None,
            }),
            "kb_read" => Some(Schema {
                description: "Read a page.".to_string(),
                parameters: r#"{"type":"object"}"#.to_string(),
                result: Some(r#"{"kb","path","text"} "#.to_string()),
            }),
            _ => None,
        }
    }
}

fn settled(status: NestedStatus) -> Settled {
    Settled::without_result(status, None)
}

#[test]
fn a_result_goes_to_stdout_as_one_json_line_or_text() {
    let texts = texts();
    let json = Settled {
        data: Some(serde_json::json!({"links": [{"title": "中文"}]})),
        text: "1 result".to_string(),
        ..settled(NestedStatus::Ok)
    };
    assert_eq!(
        settle(&Port, &texts, "web_search", Nested::Settled(json)),
        Reply {
            code: 0,
            stdout: "{\"links\":[{\"title\":\"中文\"}]}\n".to_string(),
            stderr: String::new(),
        },
        "有 data 的印一行 JSON，中文不转义"
    );
    let text = Settled {
        text: "# Page".to_string(),
        images_dropped: true,
        ..settled(NestedStatus::Ok)
    };
    assert_eq!(
        settle(&Port, &texts, "web_fetch", Nested::Settled(text)),
        Reply {
            code: 0,
            stdout: "# Page\n".to_string(),
            stderr: "Images were not passed to the script.\n".to_string(),
        }
    );
}

#[test]
fn each_ending_has_its_exit_code() {
    let texts = texts();
    let code = |nested| settle(&Port, &texts, "web_search", nested);
    let failed = Settled {
        text: "timed out".to_string(),
        ..settled(NestedStatus::Error)
    };
    assert_eq!(
        code(Nested::Settled(failed)),
        Reply::said(1, "timed out\n".to_string())
    );
    let denied =
        Settled::without_result(NestedStatus::Denied, Some("outside the workspace".into()));
    assert_eq!(
        code(Nested::Settled(denied)),
        Reply::said(1, "outside the workspace\n".to_string())
    );
    assert_eq!(
        code(Nested::Settled(settled(NestedStatus::NeedsApproval))),
        Reply::said(
            4,
            "This call needs approval, which a script cannot ask for. Call web_search directly.\n"
                .to_string()
        )
    );
    assert_eq!(
        code(Nested::Settled(settled(NestedStatus::Cancelled))),
        Reply::said(3, "Interrupted.\n".to_string())
    );
    assert_eq!(
        code(Nested::Refused(NestedRefusal::Gone)),
        Reply::said(
            3,
            "The command this call belongs to has ended.\n".to_string()
        )
    );
}

#[test]
fn usage_mistakes_say_how_to_fix_them() {
    let texts = texts();
    assert_eq!(
        settle(
            &Port,
            &texts,
            "web_serch",
            Nested::Refused(NestedRefusal::Unknown)
        ),
        Reply::said(
            2,
            "No tool named web_serch. Did you mean web_search?\n".to_string()
        ),
        "差一两个字的指出来"
    );
    assert_eq!(
        settle(
            &Port,
            &texts,
            "zzz",
            Nested::Refused(NestedRefusal::Unknown)
        )
        .stderr,
        "No tool named zzz.\n"
    );
    assert_eq!(
        settle(
            &Port,
            &texts,
            "chat_send",
            Nested::Refused(NestedRefusal::NotAllowed)
        )
        .stderr,
        "miyu call does not work for chat_send; call it directly. Tools that work with miyu call: kb_read, web_search.\n"
    );
    assert_eq!(
        settle(&Port, &texts, "web_search", Nested::Refused(NestedRefusal::NotAnObject)),
        Reply::said(
            2,
            "Bad args for web_search: not a JSON object.\nweb_search: Search the web.\nParameters: {\"type\":\"object\"}\n"
                .to_string()
        ),
        "参数不对的跟上这件的格式，省得再查一次"
    );
}

#[test]
fn schema_prints_the_description_and_parameters() {
    let texts = texts();
    assert_eq!(
        schema(&Port, &texts, "web_search").stdout,
        "web_search: Search the web.\nParameters: {\"type\":\"object\"}\n"
    );
    assert_eq!(
        schema(&Port, &texts, "kb_read").stdout,
        "kb_read: Read a page.\nParameters: {\"type\":\"object\"}\nResult: {\"kb\",\"path\",\"text\"}\n",
        "有结构化结果的跟一行样子，照原样"
    );
    assert_eq!(schema(&Port, &texts, "web_serch").code, 2);
    assert!(
        schema(&Port, &texts, "chat_send")
            .stderr
            .starts_with("miyu call does not work for chat_send")
    );
}

/// 会话里的 `miyu pkg` 调的端口（施工 BS-1 中）：`command` 交回给定的结局，`call` 调不到这里。
struct Commands(Nested);

impl NestedPort for Commands {
    fn call(&self, _name: String, _args: String) -> Calling<'_> {
        unreachable!("miyu pkg 只走 command")
    }

    fn command(&self, _name: String, _args: String) -> Calling<'_> {
        let nested = self.0.clone();
        Box::pin(async move { nested })
    }

    fn adopt(&self, _job: JobId) {}

    fn callable(&self) -> Vec<String> {
        vec!["web_search".to_string()]
    }

    fn schema(&self, _name: &str) -> Option<Schema> {
        None
    }
}

/// 没有 `packages` 包的会话（内核答不认识、不准）说这里用不了，不列 `miyu call` 能调的；收下了的照平常的结局写。
#[tokio::test]
async fn miyu_pkg_says_it_does_not_work_where_packages_is_off() {
    let texts = texts();
    for refusal in [NestedRefusal::Unknown, NestedRefusal::NotAllowed] {
        let port = Commands(Nested::Refused(refusal));
        let reply = command(&port, &texts, "package".to_string(), "{}".to_string()).await;
        assert_eq!(
            reply,
            Reply::said(
                USAGE,
                "miyu pkg does not work in this session.\n".to_string()
            )
        );
    }
    let ok = Settled {
        text: "Installed a.".to_string(),
        ..settled(NestedStatus::Ok)
    };
    let reply = command(
        &Commands(Nested::Settled(ok)),
        &texts,
        "package".to_string(),
        "{}".to_string(),
    )
    .await;
    assert_eq!((reply.code, reply.stdout.as_str()), (0, "Installed a.\n"));
    let asked = Commands(Nested::Settled(settled(NestedStatus::NeedsApproval)));
    let reply = command(&asked, &texts, "package".to_string(), "{}".to_string()).await;
    assert_eq!(reply.code, 4);
}
