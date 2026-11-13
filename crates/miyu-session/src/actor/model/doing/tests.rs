//! 回合开始交「她现在在干嘛」（施工 LF-7）：子代理不问；没人格、没端口的不问；端口没有的不交；这一段上下文里最近一块记号一样
//! 的不交，不一样的、没有的交；别的模块、别的类别的同记号块不算。

use std::sync::{Arc, Mutex, PoisonError};

use miyu_kernel::facts::Present;
use miyu_kernel::id::{FactKind, ModuleId, SessionId};
use miyu_kernel::time::UtcOffset;
use miyu_tool::{DoingLine, HooksPort, Injecting, TurnAsk};

use super::doing;

/// 只答「在干嘛」的端口，记下问了谁。
#[derive(Default)]
struct Port {
    line: Option<DoingLine>,
    asked: Mutex<Vec<(String, String)>>,
}

impl HooksPort for Port {
    fn turn_start(&self, _: TurnAsk) -> Injecting<'_> {
        Box::pin(async { Vec::new() })
    }

    fn doing(&self, persona: &str, session: &str, _: UtcOffset) -> Option<DoingLine> {
        self.asked
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push((persona.to_string(), session.to_string()));
        self.line.clone()
    }
}

fn session() -> SessionId {
    SessionId::parse("01a17000-0000-7000-8000-000000000001").unwrap()
}

fn line(key: &str) -> DoingLine {
    DoingLine {
        text: format!("<life-doing since=\"x\" until=\"y\">{key}</life-doing>\n"),
        key: key.to_string(),
    }
}

fn present(module: &str, kind: &str, key: &str) -> Present {
    Present {
        module: ModuleId::parse(module).unwrap(),
        kind: FactKind::parse(kind).unwrap(),
        refs: vec![key.to_string()],
    }
}

fn port(line: Option<DoingLine>) -> Arc<Port> {
    Arc::new(Port {
        line,
        asked: Mutex::default(),
    })
}

fn asked(port: &Port) -> Vec<(String, String)> {
    port.asked
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone()
}

#[test]
fn a_new_line_is_injected_under_life_doing_with_its_key() {
    let port = port(Some(line("100")));
    let dyn_port: Arc<dyn HooksPort> = port.clone();
    let injected = doing(
        Some((dyn_port, session())),
        (Some("muse"), false),
        UtcOffset::UTC,
        &[],
    )
    .expect("交");
    assert_eq!(injected.module.as_str(), "life");
    assert_eq!(injected.fact.kind.as_str(), "doing");
    assert_eq!(injected.fact.refs, ["100"]);
    assert_eq!(injected.fact.text, line("100").text);
    assert!(!injected.fact.after, "排在触发的那句前面");
    assert_eq!(
        asked(&port),
        [("muse".to_string(), session().as_str().to_string())]
    );
}

#[test]
fn the_same_key_as_the_latest_block_is_not_injected_again() {
    let dyn_port = || -> Arc<dyn HooksPort> { port(Some(line("200"))) };
    let ask = |present: &[Present]| {
        doing(
            Some((dyn_port(), session())),
            (Some("muse"), false),
            UtcOffset::UTC,
            present,
        )
    };
    assert!(
        ask(&[present("life", "doing", "200")]).is_none(),
        "交过的不交"
    );
    assert!(
        ask(&[
            present("life", "doing", "200"),
            present("life", "doing", "100")
        ])
        .is_some(),
        "只比最近那一块"
    );
    assert!(
        ask(&[present("life", "doing", "100")]).is_some(),
        "变了的交"
    );
    assert!(
        ask(&[
            present("memory", "doing", "200"),
            present("life", "memory", "200")
        ])
        .is_some(),
        "别的模块、别的类别不算"
    );
}

#[test]
fn subagents_sessions_without_a_persona_or_a_port_and_empty_answers_get_nothing() {
    let port_with = port(Some(line("100")));
    let dyn_port: Arc<dyn HooksPort> = port_with.clone();
    assert!(
        doing(
            Some((dyn_port.clone(), session())),
            (Some("muse"), true),
            UtcOffset::UTC,
            &[]
        )
        .is_none(),
        "子代理"
    );
    assert!(
        doing(
            Some((dyn_port, session())),
            (None, false),
            UtcOffset::UTC,
            &[]
        )
        .is_none(),
        "没人格"
    );
    assert!(asked(&port_with).is_empty(), "这两种连问都不问");
    assert!(
        doing(None, (Some("muse"), false), UtcOffset::UTC, &[]).is_none(),
        "没端口"
    );
    let empty: Arc<dyn HooksPort> = port(None);
    assert!(
        doing(
            Some((empty, session())),
            (Some("muse"), false),
            UtcOffset::UTC,
            &[]
        )
        .is_none(),
        "端口没有的不交"
    );
}
