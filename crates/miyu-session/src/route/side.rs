//! 旁路请求照声明的档位用池（施工 Q-2，设计 `43-旁路请求和档位池.md` 第三节第 3 条，`miyu_models::side`）：会话端口发的辅助
//! 请求里登记过的（现在是起标题），先照登记的档位从固定池里挑一个成员发，不动会话钉着的成员、不粘（旁路请求短、不靠缓存）。
//!
//! - 池是空的（一个成员都认不出）：用会话的模型，不算退。
//! - 池里挑一个发；还没收到增量就出了会冷却的错（限速、可重试、认证失败）的，换下一个。内核不替辅助请求重发，所以在这里、
//!   派出去的那个任务里换。挑到这一次试过的、挑不出能发的（全在冷却、备不好）：退到会话的模型发一次，记一行
//!   `INFO side fallback`，`model.called` 写 `fell_back`。回退只在出字以前（`models.md`「五」第 4 条）。
//! - 会话的模型本身就是这个池的：不再退，照会话的模型发那一次、照池的规矩报（免得来回退）。

use std::sync::{Arc, PoisonError};

use tracing::Instrument;

use miyu_kernel::event::Purpose;
use miyu_kernel::request::Request;
use miyu_models::cooldown::Candidate;
use miyu_models::side::{self, Side};

use super::base::Seat;
use super::choice::{Unsent, name};
use super::send::{self, Attempt, Chosen, Stop, Tried};
use super::{Route, TARGET, refused};
use crate::config::TurnConfig;
use crate::port::{Cancel, Reports};

/// 照档位挑的结果。
pub(super) enum Picked {
    /// 池里挑出了能发的。
    Served(Box<Chosen>),
    /// 池是空的：用会话的模型，不算退。
    Empty,
    /// 池挑不出能发的：原话。
    Unavailable(String),
}

/// 辅助请求的用途是哪一条登记；没登记的（回顾、提前压的摘要：复用会话前缀的缓存）照会话的模型。
pub(super) fn registered(purpose: Option<&Purpose>) -> Option<&'static Side<'static>> {
    match purpose {
        Some(Purpose::Title) => Some(&side::TITLE),
        _ => None,
    }
}

impl Route {
    /// 照登记 `side` 的档位，从固定池里挑一个成员，照这一轮的配置 `config`。种子是会话编号，不看钉着的、不粘。
    pub(super) fn side(&self, config: &TurnConfig, side: &Side<'_>) -> Picked {
        let values = config.resolved.values();
        let routes = &self.shared;
        let Ok(resolved) = routes.resolve(&values, &side::reference(side)) else {
            return Picked::Empty;
        };
        let seat = Seat {
            seed: self.session.as_str(),
            held: None,
            sticky: None,
            used: None,
        };
        let (picked, _) = match routes.pick(config, &values, &resolved, &seat) {
            Ok(picked) => picked,
            Err(Unsent::NoModel(no_model)) => return Picked::Unavailable(no_model.0),
            Err(Unsent::Cooling { message, .. } | Unsent::TooLong(message)) => {
                return Picked::Unavailable(message);
            }
        };
        let texts = self
            .texts
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        let ready = match routes.ready(config, &picked.choice, texts, None) {
            Ok(ready) => ready,
            Err(no_model) => return Picked::Unavailable(no_model.0),
        };
        Picked::Served(Box::new(Chosen {
            ready,
            blobs: self.blobs.clone(),
            tried: Tried {
                picked,
                pinned: Arc::clone(&self.pinned),
                config: Arc::clone(config),
                main: false,
                pins: false,
            },
        }))
    }

    /// 会话钉着的引用就是登记 `side` 的那个池：退不出去。
    pub(super) fn rides(&self, side: &Side<'_>) -> bool {
        self.lock().reference.as_deref() == Some(side::reference(side).as_str())
    }
}

/// 在派出去的任务里照登记 `side` 发这一次辅助请求，带着当前的 span。
pub(super) fn spawn(
    route: Arc<Route>,
    side: &'static Side<'static>,
    config: &TurnConfig,
    request: Request,
    reports: Reports,
    cancel: Cancel,
) {
    let config = Arc::clone(config);
    tokio::spawn(
        ask(route, side, config, request, reports, cancel).instrument(tracing::Span::current()),
    );
}

/// 照档位池换着发；池是空的用会话的模型，池里都不行退到会话的模型（见模块的说明）。
async fn ask(
    route: Arc<Route>,
    side: &'static Side<'static>,
    config: TurnConfig,
    request: Request,
    mut reports: Reports,
    cancel: Cancel,
) {
    let mut stop: Stop = Box::pin(cancel.wait());
    let mut tried: Vec<Candidate> = Vec::new();
    let why = loop {
        match route.side(&config, side) {
            Picked::Served(chosen) if tried.contains(&chosen.tried.picked.choice.who) => {
                break format!("{} failed", name(&chosen.tried.picked.choice.who));
            }
            Picked::Served(chosen) => {
                let who = chosen.tried.picked.choice.who.clone();
                match send::attempt(*chosen, &request, reports, &mut stop, true).await {
                    Attempt::Reported => return,
                    Attempt::Retry(back) => {
                        reports = *back;
                        tried.push(who);
                    }
                }
            }
            Picked::Empty if tried.is_empty() => {
                return session(&route, &config, &request, reports, &mut stop).await;
            }
            Picked::Empty => break "the pool became empty".to_string(),
            Picked::Unavailable(why) => break why,
        }
    };
    if route.rides(side) {
        return session(&route, &config, &request, reports, &mut stop).await;
    }
    tracing::info!(target: TARGET, side = side.id, why, "side fallback");
    session(&route, &config, &request, reports.fell_back(), &mut stop).await;
}

/// 照会话的模型发一次（不换人：退过来的这一次就是最后一次）。
async fn session(
    route: &Route,
    config: &TurnConfig,
    request: &Request,
    reports: Reports,
    stop: &mut Stop,
) {
    match route.choose(config, false, None) {
        Ok(chosen) => {
            send::attempt(chosen, request, reports, stop, false).await;
        }
        Err(error) => refused(reports, error),
    }
}
