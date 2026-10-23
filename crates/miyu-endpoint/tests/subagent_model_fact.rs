//! 子代理的模型那一块事实（施工 E-M 修，Windows 上撞见）：子代理一造好就开第一轮，那时交给内核的限额要已经补上预设，第一块
//! 模型事实就写对的预设，不先写一块空的再补一块。真核心走一遍：主会话派一个子代理，子代理的日志里模型那一块只有一条。

use std::sync::Arc;

use miyu_kernel::event::Body;
use miyu_kernel::facts::Identity;
use miyu_kernel::id::Seq;
use miyu_kernel::origin::Model;
use miyu_kernel::request::Request;
use miyu_kernel::session::Limits;
use miyu_session::testkit::{Play, Script};
use miyu_session::{Cancel, ForSession, ModelPort, Models, Reports, TurnConfig};

use crate::support::deleting::*;
use crate::support::*;

/// 照剧本分、报得出这一轮发给谁的端口：路由那样交身份（预设留空，等会话补）。
#[derive(Clone)]
struct Named(Router);

impl Models for Named {
    fn port(&self, _: ForSession) -> Arc<dyn ModelPort> {
        Arc::new(self.clone())
    }
}

impl ModelPort for Named {
    fn model(&self) -> Model {
        self.0.model()
    }

    fn limits(&self) -> Limits {
        Limits {
            model: self.model(),
            window: None,
            max_output: None,
            images: None,
            blind: false,
            identity: Some(Identity {
                provider: "dev".to_string(),
                model: "flash".to_string(),
                effort: "default".to_string(),
                preset: String::new(),
            }),
        }
    }

    fn call(
        &self,
        seen: Seq,
        request: Request,
        config: &TurnConfig,
        reports: Reports,
        cancel: Cancel,
    ) {
        self.0.call(seen, request, config, reports, cancel);
    }
}

/// 日志里内核记的模型那几块的原文。
fn model_facts(log: &[miyu_kernel::event::Event]) -> Vec<String> {
    log.iter()
        .filter_map(|event| match &event.body {
            Body::ContextInjected(fact) if fact.kind.as_str() == "model" => Some(fact.text.clone()),
            _ => None,
        })
        .collect()
}

#[tokio::test]
async fn a_subagent_tells_its_model_once_with_the_preset() {
    let home = Home::new();
    let router = Router(Arc::new(vec![
        (
            "派一个去查",
            Script::new([
                Play::calls(&[("subagent", &agent("查 A"))]),
                Play::Says("派出去了。"),
            ]),
        ),
        ("查 A", Script::new([Play::Says("查完了。")])),
    ]));
    let tools =
        miyu_tool::Catalog::new(miyu_basesystem::tools(&default_resources()).expect("出厂的工具"))
            .expect("合写法");
    let mut client = Client::connect(home.core_with_models(Arc::new(Named(router)), tools));
    client.hello().await;
    let work = home.work.to_string_lossy().into_owned();
    let parent = client.create("c1", &work).await;
    client.say("c2", &parent, "派一个去查").await;
    home.until_turns(&parent, 1).await;
    let child = started_child(&home.log(&parent)).expect("派出去了子代理");
    home.until_turns(child.as_str(), 1).await;
    let told = model_facts(&home.log(child.as_str()));
    assert_eq!(told.len(), 1, "子代理的模型那一块只注入一次：{told:?}");
    assert!(told[0].contains(r#"preset="full""#), "{told:?}");
    let parent_told = model_facts(&home.log(&parent));
    assert_eq!(parent_told.len(), 1, "{parent_told:?}");
    assert!(
        parent_told[0].contains(r#"preset="full""#),
        "{parent_told:?}"
    );
}
