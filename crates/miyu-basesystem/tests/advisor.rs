//! `advisor`（`docs/blueprint/tools/advisor.md`，设计 69，施工 ADV-2）：声明问题和 `continue`，说明是设计里的原文、是常量（不随
//! 旗舰池有没有模型变）；经端口问，交回回答原文、顾问会话的编号给头；旗舰池没有模型的交回那一句；顾问没答成的照原因说；
//! 没有端口的照问不了说；参数不对的照共用的那一句、端口不问。给人看的说法、显示名三种语言里都有。

use std::sync::{Arc, Mutex, PoisonError};

use serde_json::json;

use miyu_kernel::block::{Block, Text};
use miyu_kernel::id::SessionId;
use miyu_kernel::tool::Access;
use miyu_store::human::Human;
use miyu_store::resources::ResourceRoot;
use miyu_tool::{Advised, Advising, AgentPort, Done, NotAdvised, NotSpawned, Order, Spawning};

use crate::support::{Site, check, human, readable, resources, said, tool};

/// 顾问会话的编号。
const ADVISOR: &str = "01a0d78c-ca52-7d19-8b64-0e3f5a7c2d93";

/// 设计 69 第三节第 2 条的原文。
const DESCRIPTION: &str = "Ask a stronger model for advice when you are stuck. It sees only your question, so include the context it needs.";

/// 假的端口：记下每一次的问题和 `continue`，照 `answer` 答。
struct Port {
    answer: Result<Advised, NotAdvised>,
    asked: Mutex<Vec<(String, bool)>>,
}

impl Port {
    fn new(answer: Result<Advised, NotAdvised>) -> Arc<Port> {
        Arc::new(Port {
            answer,
            asked: Mutex::new(Vec::new()),
        })
    }

    fn answering(text: &str) -> Arc<Port> {
        Port::new(Ok(Advised {
            session: SessionId::parse(ADVISOR).expect("测试的数据合写法"),
            answer: text.to_string(),
        }))
    }

    fn asked(&self) -> Vec<(String, bool)> {
        self.asked
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

impl AgentPort for Port {
    fn spawn<'a>(&'a self, _order: Order<'a>) -> Spawning<'a> {
        Box::pin(async { Err(NotSpawned) })
    }

    fn pools(&self) -> &[String] {
        &[]
    }

    fn personas(&self) -> &[String] {
        &[]
    }

    fn advise<'a>(&'a self, question: &'a str, continued: bool) -> Advising<'a> {
        self.asked
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push((question.to_string(), continued));
        let answer = self.answer.clone();
        Box::pin(async move { answer })
    }
}

/// 交回的那一段字。
fn text(done: &Done) -> &str {
    match done.blocks.as_slice() {
        [Block::Text(Text { text })] => text,
        other => panic!("只有一段字：{other:?}"),
    }
}

#[test]
fn it_declares_a_question_and_continue_with_the_designed_words() {
    let advisor = tool("advisor");
    let spec = advisor.spec();
    assert_eq!(spec.name, "advisor");
    // 问一句什么都不改：一步里调几次连着一起问，只读的时候也问得了。
    assert_eq!(spec.access, Access::Read);
    assert_eq!(spec.description, DESCRIPTION);
    let parameters: serde_json::Value =
        serde_json::from_str(spec.parameters.get()).expect("测试的数据合写法");
    assert_eq!(
        parameters,
        json!({
            "type": "object",
            "properties": {
                "question": {"type": "string"},
                "continue": {
                    "type": "boolean",
                    "description": "true: continue your last advisor conversation in this session."
                }
            },
            "required": ["question"]
        })
    );
}

/// 工具面会话内字节恒定（设计 69 第二节）：说明、参数格式是资源里的常量，造几次都一字不差，里面没有池、模型的名字。
#[test]
fn its_words_do_not_depend_on_the_flagship_pool() {
    let first = tool("advisor");
    let again = tool("advisor");
    assert_eq!(first.spec().description, again.spec().description);
    assert_eq!(first.spec().parameters.get(), again.spec().parameters.get());
    let words = format!(
        "{}{}",
        first.spec().description,
        first.spec().parameters.get()
    );
    assert!(
        !words.contains("flagship") && !words.contains('@'),
        "{words}"
    );
}

#[tokio::test]
async fn the_answer_comes_back_as_it_was_said() {
    let site = Site::new();
    let answer = "Check the lock order first.\nThen retry.";
    let port = Port::answering(answer);
    let question = "Two threads deadlock on A and B.\nWhat should I check?";
    let done = site
        .done_with_agents("advisor", json!({"question": question}), Some(port.clone()))
        .await;
    assert!(!done.error);
    assert_eq!(text(&done), answer, "回答原文，末尾不加东西");
    assert_eq!(port.asked(), [(question.to_string(), false)]);
    assert_eq!(
        done.detail,
        Some(json!({"session": ADVISOR})),
        "顾问会话的编号给头点进去看"
    );
}

#[tokio::test]
async fn continue_goes_to_the_port() {
    let site = Site::new();
    let port = Port::answering("Yes.");
    for args in [
        json!({"question": "Q1", "continue": true}),
        json!({"question": "Q2", "continue": false}),
        json!({"question": "Q3", "continue": null}),
    ] {
        let done = site
            .done_with_agents("advisor", args, Some(port.clone()))
            .await;
        assert!(!done.error, "{}", text(&done));
    }
    assert_eq!(
        port.asked(),
        [
            ("Q1".to_string(), true),
            ("Q2".to_string(), false),
            ("Q3".to_string(), false)
        ]
    );
}

#[tokio::test]
async fn an_empty_flagship_pool_is_told_plainly() {
    let site = Site::new();
    let port = Port::new(Err(NotAdvised::NoModel));
    let done = site
        .done_with_agents("advisor", json!({"question": "Q"}), Some(port))
        .await;
    assert!(done.error);
    assert_eq!(text(&done), "No model is set in the flagship pool.\n");
    assert!(done.detail.is_none());
}

#[tokio::test]
async fn a_failed_advisor_or_no_port_says_why() {
    let site = Site::new();
    let port = Port::new(Err(NotAdvised::Failed(
        "error: upstream returned 503".to_string(),
    )));
    let done = site
        .done_with_agents("advisor", json!({"question": "Q"}), Some(port))
        .await;
    assert!(done.error);
    assert_eq!(
        text(&done),
        "The advisor could not answer: error: upstream returned 503\n"
    );
    let done = site
        .done_with_agents("advisor", json!({"question": "Q"}), None)
        .await;
    assert!(done.error);
    assert_eq!(
        text(&done),
        "The advisor could not answer: no advisor here\n"
    );
}

#[tokio::test]
async fn without_a_question_nothing_is_asked() {
    let site = Site::new();
    let port = Port::answering("Yes.");
    for args in [json!({}), json!({"question": 1}), json!({"continue": true})] {
        let done = site
            .done_with_agents("advisor", args, Some(port.clone()))
            .await;
        assert!(done.error);
        assert!(
            text(&done).starts_with("Bad arguments: "),
            "{}",
            text(&done)
        );
    }
    assert!(port.asked().is_empty());
}

#[tokio::test]
async fn every_outcome_says_something_people_can_read() {
    let site = Site::new();
    let mut checked = Vec::new();
    let ask = || json!({"question": "Q"});
    let done = site
        .done_with_agents("advisor", ask(), Some(Port::answering("A")))
        .await;
    check(
        &mut checked,
        human(done),
        said("advisor/answered").with("session", ADVISOR),
    );
    let done = site
        .done_with_agents("advisor", ask(), Some(Port::new(Err(NotAdvised::NoModel))))
        .await;
    check(&mut checked, human(done), said("advisor/no-model"));
    let done = site.done_with_agents("advisor", ask(), None).await;
    check(
        &mut checked,
        human(done),
        said("advisor/failed").with("reason", "no advisor here"),
    );
    readable(&checked, &["advisor"]);
}

/// 显示名三种语言里都有（设计 69 施工 ADV-2）：顾问、Advisor、アドバイザー。
#[test]
fn it_is_shown_as_the_advisor() {
    let root = ResourceRoot::at(resources());
    for (language, name) in [("zh", "顾问"), ("en", "Advisor"), ("ja", "アドバイザー")] {
        let words = Human::load(&root, language).expect("给人看的字读得出来");
        let shown = words.tool("advisor").expect("有 advisor 的显示名");
        assert_eq!(shown.name, name, "{language}");
    }
}
