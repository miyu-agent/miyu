//! 发给扩展提供的驱动（施工 MD-2，设计 66 第三、五、八、九节）：`route/exchange.rs` 的分岔。供应商的驱动是扩展的，不走「编码、
//! `miyu-http` 发、解码」，改成经端口（[`DriverPort`]）发 `driver.call`、收 `driver.delta`、收回答；交给挑的一方的还是「发出去了」、
//! 同一串 [`Delta`](miyu_kernel::accumulate::Delta) 和同一个结尾，重试、冷却、换池照内置驱动一样走（`route/ended.rs`）。
//!
//! - 挑候选（[`Routes::extension_choices`]）：驱动不在的（端口说的原因）当场 `no_model`，池里跳过；key 照驱动的声明取
//!   （`none` 的不取，`required` 的没写当 `no_model`），地址是这一家写的，没写的不交。
//! - 交过去的：别家驱动的私有数据拿掉；声明了看得了图的，图照原样交；参数的写法在 `extension/wire.rs`。
//! - 增量照内核的累加器先查一遍：不对的不要了（端口发 `driver.cancel`），当 `unclassified`，记一行 `WARN driver bad delta`。
//! - 时限同内置驱动走 HTTP：发出去到第一条增量、两条增量之间、最后一条到回答都不超过空闲时限，整个不超过它的 `TOTAL_FACTOR` 倍；
//!   超了的不要了，当 `retryable`。连接断了也当 `retryable`。
//! - 回答写坏了的当 `unclassified`，记一行 `WARN driver bad answer`；回答里的 `sent` 随说完了交上去，记进 `model.called`。

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::future::Future;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use base64::Engine;
use serde_json::Value;
use tokio::sync::mpsc;
use tokio::time::Instant;

use miyu_config::package::DriverKey;
use miyu_config::secret::{Reference, Secret};
use miyu_drivers::classify::Classified;
use miyu_http::{Endpoint, Progress, TOTAL_FACTOR};
use miyu_kernel::accumulate::Accumulator;
use miyu_kernel::event::ErrorClass;
use miyu_kernel::id::{ContentHash, DriverFamily};
use miyu_kernel::request::Request;
use miyu_models::cooldown::Candidate;
use miyu_models::pools::Member;
use miyu_models::provider::{self, DriverRef, NoModel, Provider, Target};
use miyu_store::blob::Blobs;

use super::Routes;
use super::base::Ready;
use super::choice::Choice;
use super::driver_port::{Broken, DriverPort, NotYet};
use super::exchange::{Exchanged, capped, missing};
use super::shared::ModelData;
use crate::TARGET;
use crate::blocking::blocking;
use crate::config::TurnConfig;

pub mod check;
mod listing;
mod wire;
mod wires;

pub(super) use listing::listed;
pub(super) use wires::Wires;

use wire::{Outgoing, Picture, Said};

/// 挑候选时备下的（扩展驱动的那一家）：看不看得了图，这一家的 key（照驱动的声明，`none` 的没有），这一家写的地址。
#[derive(Debug, Clone)]
pub(super) struct Reach {
    pub(super) images: bool,
    pub(super) key: Option<Secret>,
    pub(super) base_url: Option<String>,
}

/// 备好了的扩展驱动：哪一个、经哪个端口、挑候选时备下的。
pub(super) struct Reached {
    pub(super) driver: DriverRef,
    pub(super) port: Arc<dyn DriverPort>,
    pub(super) reach: Reach,
}

impl Routes {
    /// 驱动 `driver` 是扩展提供的那一家（`target`）的候选：见模块的说明。`member` 是池里的哪一个成员。
    pub(super) fn extension_choices(
        &self,
        config: &TurnConfig,
        target: &Target,
        member: Option<&Member>,
        driver: &DriverRef,
    ) -> Result<Vec<Choice>, NoModel> {
        let provider = &target.provider;
        let reach = reach(&self.data, provider, driver, &|reference| {
            config.secret(reference)
        })?;
        Ok(vec![Choice {
            who: Candidate::new(&provider.id, &target.model),
            target: target.clone(),
            endpoint: Endpoint::keyless(reach.base_url.clone().unwrap_or_default()),
            member: member.cloned(),
            extension: Some(reach),
        }])
    }
}

/// 扩展驱动 `driver` 那一家（`provider`）要交给扩展的：问端口在不在、声明了什么（不在的当场 `no_model`）；key 照声明取（`none` 的
/// 不取，`required` 的没写当 `no_model`，写了取不到的也是）；地址是这一家写的，扩展驱动的供应商可以不写（`provider()` 记成空的），
/// 不写的不交（第九节第 1 条）。挑候选、拉模型列表都照它（施工 MD-4 抽出来）。
pub(super) fn reach(
    data: &ModelData,
    provider: &Provider,
    driver: &DriverRef,
    secret: &dyn Fn(&Reference) -> Option<Secret>,
) -> Result<Reach, NoModel> {
    let port = data
        .drivers()
        .ok_or_else(|| NoModel(super::unbuilt(provider)))?;
    let declared = port
        .present(driver)
        .map_err(|why| NoModel(format!("model driver \"{driver}\" is unavailable: {why}")))?;
    let usable = || NoModel(format!("provider {:?} has no usable key", provider.id));
    let key = match (declared.key, &provider.key) {
        (DriverKey::None, _) | (DriverKey::Optional, None) => None,
        (DriverKey::Required, None) => return Err(usable()),
        (_, Some(reference)) => Some(secret(reference).ok_or_else(usable)?),
    };
    let base_url = provider::resolve_base_url(provider, secret)?;
    Ok(Reach {
        images: declared.images,
        key,
        base_url: (!base_url.is_empty()).then_some(base_url),
    })
}

/// 这一次调用的编号：同一个核心里不重。
fn next_call() -> String {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    format!("d{}", NEXT.fetch_add(1, Ordering::Relaxed) + 1)
}

/// 发一次（同 [`super::exchange::exchange`]，见模块的说明）。
pub(super) async fn exchange(
    ready: &Ready,
    reached: &Reached,
    request: Cow<'_, Request>,
    blobs: &Blobs,
    cancel: impl Future<Output = ()> + Send,
    mut on: impl FnMut(Progress) + Send,
) -> Exchanged {
    let name = reached.driver.to_string();
    // 清单查过包编号、驱动名的写法，家族一定认得；万一认不得，照驱动的错当场说完，不发。
    let Ok(family) = DriverFamily::parse(&name) else {
        return said(wire::classified(
            ErrorClass::Unclassified,
            &format!("model driver \"{name}\" has a name that can't own private data"),
            None,
        ));
    };
    // 别家驱动的私有数据只回给它自己（设计 66 第四节第 3 条）：有的拷一份拿掉，没有的原样交。
    let request = match wire::foreign(&request, &family) {
        false => request,
        true => {
            let mut copy = request.into_owned();
            wire::strip(&mut copy, &family);
            Cow::Owned(copy)
        }
    };
    let pictures = match reached.reach.images {
        false => BTreeMap::new(),
        true => match pictures(blobs, wire::images(&request)).await {
            Ok(pictures) => pictures,
            Err(hash) => return Exchanged::Missing(missing(&hash)),
        },
    };
    let call = capped(&ready.call, request.output_cap);
    let bytes = request.canonical_bytes();
    let fingerprint = request.fingerprint();
    drop(request);
    let id = next_call();
    let params = wire::params(&Outgoing {
        call: &id,
        driver: &name,
        request: &bytes,
        fingerprint: &fingerprint,
        params: call.as_ref().unwrap_or(&ready.call),
        key: reached.reach.key.as_ref().map(Secret::expose),
        base_url: reached.reach.base_url.as_deref(),
        blobs: &pictures,
    });
    let hash = ContentHash::of(&bytes);
    drop((bytes, pictures));
    let (tell, mut heard) = mpsc::unbounded_channel();
    let pending = match reached.port.call(&reached.driver, &id, params, tell) {
        Ok(pending) => pending,
        Err(NotYet { retry_in_ms }) => {
            return said(wire::classified(
                ErrorClass::Retryable,
                &format!("model driver \"{name}\" isn't connected yet"),
                retry_in_ms,
            ));
        }
    };
    on(Progress::Sent {
        request: hash.clone(),
        at: std::time::Instant::now(),
    });
    let idle = ready.idle;
    let started = Instant::now();
    let deadline = started + idle * TOTAL_FACTOR;
    let mut last = started;
    let mut checker = Accumulator::default();
    let mut closed = false;
    tokio::pin!(pending, cancel);
    loop {
        let quiet = tokio::time::sleep_until((last + idle).min(deadline));
        tokio::select! {
            biased;
            () = &mut cancel => return Exchanged::Cancelled,
            batch = heard.recv(), if !closed => match batch {
                Some(batch) => {
                    last = Instant::now();
                    if let Err(why) = take(batch, &family, &mut checker, &mut on) {
                        return bad(&name, "delta", &why);
                    }
                }
                // 端口放掉了交增量的那一头：连接断了，回答那一头跟着了结。
                None => closed = true,
            },
            answered = &mut pending => {
                // 回答之前到的增量都已经交进来了（同一条连接，端口先交增量再交回答）：收尾前读空。
                while let Ok(batch) = heard.try_recv() {
                    if let Err(why) = take(batch, &family, &mut checker, &mut on) {
                        return bad(&name, "delta", &why);
                    }
                }
                return finish(answered, &name, &hash, &ready.learn.data);
            }
            () = quiet => {
                // 多久没动静的是卡住（施工 YC-3）；增量一直在来、过了总时限的不算，照旧可重试。
                let class = match Instant::now() >= deadline {
                    true => ErrorClass::Retryable,
                    false => ErrorClass::Stalled,
                };
                return said(wire::classified(class, "the driver went quiet", None));
            }
        }
    }
}

/// 收下一份增量：读、照累加器查，一条条交出去。
fn take(
    batch: Value,
    family: &DriverFamily,
    checker: &mut Accumulator,
    on: &mut impl FnMut(Progress),
) -> Result<(), String> {
    for delta in wire::deltas(batch, family)? {
        checker
            .apply(delta.clone())
            .map_err(|error| error.to_string())?;
        on(Progress::Delta(delta));
    }
    Ok(())
}

/// 等到了回答（或者等不到了）。
fn finish(
    answered: Result<Value, Broken>,
    name: &str,
    request: &ContentHash,
    data: &ModelData,
) -> Exchanged {
    match answered {
        Err(Broken::Gone) => said(wire::classified(
            ErrorClass::Retryable,
            "the driver went away",
            None,
        )),
        Err(Broken::Refused(message)) => said(wire::classified(
            ErrorClass::Unclassified,
            &format!("the driver refused the call: {message}"),
            None,
        )),
        Ok(result) => match wire::answer(result) {
            Ok(Said { usage, error, sent }) => {
                // 同一个请求编出了不同的字节（施工 MD-3，设计 66 第七节第 3 条）：缓存前缀命中不了，每个驱动每次运行说一次。
                let before = sent
                    .as_ref()
                    .and_then(|sent| data.wires().note(name, request, sent));
                if let (Some(before), Some(sent)) = (before, &sent) {
                    tracing::warn!(target: TARGET, driver = name, request = %request, before = %before, sent = %sent, "driver encoded the same request differently");
                }
                Exchanged::Ended {
                    usage,
                    error,
                    wire: sent,
                }
            }
            Err(why) => bad(name, "answer", &why),
        },
    }
}

/// 驱动交的东西写坏了：记一行 `WARN`，当 `unclassified`。
fn bad(name: &str, what: &str, why: &str) -> Exchanged {
    match what {
        "delta" => tracing::warn!(target: TARGET, driver = name, error = why, "driver bad delta"),
        _ => tracing::warn!(target: TARGET, driver = name, error = why, "driver bad answer"),
    }
    said(wire::classified(
        ErrorClass::Unclassified,
        &format!("the driver sent a bad {what}: {why}"),
        None,
    ))
}

/// 没有用量、出错是 `classified` 的结尾。
fn said(classified: Classified) -> Exchanged {
    Exchanged::Ended {
        usage: None,
        error: Some(classified),
        wire: None,
    }
}

/// 照原样交过去的图（声明了看得了图的驱动）：在阻塞线程里取、写成 base64。取不出来的交回是哪一个。
async fn pictures(
    blobs: &Blobs,
    wanted: BTreeMap<ContentHash, String>,
) -> Result<BTreeMap<String, Picture>, ContentHash> {
    if wanted.is_empty() {
        return Ok(BTreeMap::new());
    }
    let blobs = blobs.clone();
    blocking(move || {
        wanted
            .into_iter()
            .map(|(hash, media_type)| {
                let bytes = blobs.get(&hash).map_err(|_| hash.clone())?;
                let data = base64::engine::general_purpose::STANDARD.encode(bytes);
                Ok((hash.to_string(), Picture { media_type, data }))
            })
            .collect()
    })
    .await
}

#[cfg(test)]
mod tests;
