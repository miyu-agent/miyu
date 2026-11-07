//! 发一次（`docs/designs/05-内核接口.md` 第七节「驱动的规格」「HTTP 执行器」，施工 3-7 下；施工 8-20 从 `route/send.rs`
//! 拆出来，是底子的一块，两个入口共用）：照驱动列的清单在阻塞线程里取 blob，编码成片段清单，经 `miyu_http::send` 发出去、
//! 流式读回来，「发出去了」和每一段增量交给挑的一方。报上下文超长、说了上限、比手头的窗口小的，记下用出来的窗口（施工
//! 8-7，`models.md`「怎么走」第二条第 9 条），再交回说完了。
//!
//! 主请求编码时前面没变的历史接着用会话留着的那一份（施工 V-42，[`Memo`]）：发出去的字节和整份编的一个都不差；旁路请求、
//! 一次性的照旧整份编。
//!
//! 请求体边编边发（施工 V-38）：照原样发的图片、PDF 只核对、量大小，发的时候 HTTP 执行器才从 blob 一小块一小块地读出来编
//! （[`StoreBlobs`]）；照字放进消息的文件照旧取整份。编码完，交进来的请求（主请求那一份）、取来的字节、线上的中间结构当场
//! 放掉，等模型回答时手里不留整份请求。

use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};
use std::future::Future;
use std::io::{self, Read};
use std::sync::{Arc, Mutex};

use miyu_drivers::classify::Classified;
use miyu_drivers::{BlobBytes, Call, EncodeError, Memo};
use miyu_http::{Attempt, BlobSource, Body, Outcome, Progress, send};
use miyu_kernel::event::{CallError, ErrorClass, Usage};
use miyu_kernel::id::ContentHash;
use miyu_kernel::request::Request;
use miyu_store::blob::{BlobError, Blobs};

use super::base::{Ready, Speaker};
use crate::blocking::blocking;
use crate::clock::wall_now;
use crate::route::shared::ModelData;

/// 用出来的窗口记到哪：这一家、这个模型，和发的时候手头的窗口。
pub(super) struct Learn {
    pub(super) data: Arc<ModelData>,
    pub(super) provider: String,
    pub(super) model: String,
    pub(super) window: Option<u64>,
}

impl Learn {
    /// 报了的上限 `limit` 比手头的窗口小（或者手头没有窗口）：记下。
    async fn limit(&self, limit: u64) {
        if self.window.is_some_and(|window| window <= limit) {
            return;
        }
        let (data, provider, model) = (
            Arc::clone(&self.data),
            self.provider.clone(),
            self.model.clone(),
        );
        blocking(move || data.learn(&provider, &model, limit, wall_now())).await;
    }
}

/// 发一次怎么收场的。
pub(super) enum Exchanged {
    /// 编码要的 blob 取不出来：没发出去（分类「其他」，重试也没用）。
    Missing(CallError),
    /// 被叫停了：之后什么都没报。
    Cancelled,
    /// 说完了：用量，出错的分类、原话、要等多久（报了上限的已经记下了），真发出去的字节的哈希（扩展驱动报的，施工 MD-2；
    /// 内置驱动的没有）。
    Ended {
        usage: Option<Usage>,
        error: Option<Classified>,
        wire: Option<ContentHash>,
    },
}

/// 输出上限照请求的 `output_cap` 压（施工 6-11 三补，`Request::output_cap`）：有的，和原来的取小的；原来没写的（供应商默认）
/// 照它写，不然供应商默认的输出加上用量可能超窗口。没有的、原来就更小的交回 `None`，照原样发。
pub(super) fn capped(call: &Call, cap: Option<u64>) -> Option<Call> {
    let cap = u32::try_from(cap?).unwrap_or(u32::MAX);
    if call.max_output.is_some_and(|max| max <= cap) {
        return None;
    }
    Some(Call {
        max_output: Some(cap),
        ..call.clone()
    })
}

/// 发一次：取 blob、编码、发、读回来，「发出去了」和增量一有就交给 `on`；`cancel` 一完成就停。`request` 是交进来的那一份
/// （主请求）的，编码完就放；借来的（旁路请求、一次性的，换人还要再发）照旧借着。`memo` 是会话留着的编好的历史（主请求
/// 有，施工 V-42）：有的照它接着编，没有的整份编。
pub(super) async fn exchange(
    ready: &Ready,
    request: Cow<'_, Request>,
    memo: Option<&Mutex<Memo>>,
    blobs: &Blobs,
    cancel: impl Future<Output = ()> + Send,
    on: impl FnMut(Progress) + Send,
) -> Exchanged {
    let driver = match &ready.speaker {
        Speaker::Builtin(driver) => driver.as_ref(),
        // 扩展提供的驱动（施工 MD-2）：不编码、不走 HTTP，经端口交给扩展。
        Speaker::Extension(reached) => {
            return super::extension::exchange(ready, reached, request, blobs, cancel, on).await;
        }
    };
    let needed = driver.blobs_needed(&request, &ready.call);
    // 一个 blob 都不用的（多数请求）不跳阻塞线程：两核的机器上那一跳偶尔要等上一两毫秒，正落在投影里（施工 PERF-1）。
    let fetched = match needed.is_empty() {
        true => Fetched::default(),
        false => {
            let whole = driver.blobs_whole(&request, &ready.call);
            let blobs = blobs.clone();
            blocking(move || fetch(&blobs, needed, &whole)).await
        }
    };
    // 占位工具（施工 8-14 补）：档案点名了哪几件、工具面里缺的才拷一份补上；不缺的、没点名的原样编码，一个字节不动。
    let request = match super::placeholder::missing(&request, &ready.placeholders) {
        false => request,
        true => {
            let mut copy = request.into_owned();
            super::placeholder::fill(&mut copy, &ready.placeholders);
            Cow::Owned(copy)
        }
    };
    // 快满了的主请求，回答压到窗口剩下的（施工 6-11 三补）：拷一份改输出上限，平常的照原样。
    let call = capped(&ready.call, request.output_cap);
    let call = call.as_ref().unwrap_or(&ready.call);
    let encoded = match memo {
        Some(memo) => {
            let mut kept = memo.lock().unwrap_or_else(|poisoned| {
                // 编到一半 panic 过：留着的不一定齐，整份扔掉重来。
                let mut kept = poisoned.into_inner();
                kept.clear();
                memo.clear_poison();
                kept
            });
            driver.fragments_memo(&request, call, &fetched, &mut kept)
        }
        None => driver.fragments(&request, call, &fetched),
    };
    // 编完了：请求（交进来的那一份）、取来的字节放掉（施工 V-38）。
    drop(request);
    drop(fetched);
    let encoded = match encoded {
        Ok(encoded) => encoded,
        Err(EncodeError::MissingBlob(hash)) => return Exchanged::Missing(missing(&hash)),
    };
    let attempt = Attempt {
        client: &ready.client,
        endpoint: &ready.endpoint,
        driver,
        body: Body::new(encoded.body, Arc::new(StoreBlobs(blobs.clone()))),
        path: &encoded.path,
        idle: ready.idle,
    };
    match send(attempt, cancel, on).await {
        Outcome::Cancelled => Exchanged::Cancelled,
        Outcome::Ended { usage, error } => {
            if let Some(limit) = error.as_ref().and_then(|classified| classified.limit) {
                ready.learn.limit(limit).await;
            }
            Exchanged::Ended {
                usage,
                error,
                wire: None,
            }
        }
    }
}

/// 编码前取好的 blob（施工 V-38）：照原样发的只有大小（核对过），照字放进消息的文件有整份的字节。取不出来的（丢了、坏了、
/// 读不了）不在里面：编码时驱动报缺的是哪一个。
#[derive(Default)]
struct Fetched(BTreeMap<ContentHash, Got>);

/// 取好的一个 blob。
struct Got {
    size: u64,
    whole: Option<Vec<u8>>,
}

impl BlobBytes for Fetched {
    fn bytes(&self, hash: &ContentHash) -> Option<&[u8]> {
        self.0.get(hash)?.whole.as_deref()
    }

    fn size(&self, hash: &ContentHash) -> Option<u64> {
        self.0.get(hash).map(|got| got.size)
    }
}

/// 照清单取 blob：`whole` 里的取整份（核对哈希），别的只核对、量大小（[`Blobs::check`]），发的时候再读。
fn fetch(blobs: &Blobs, needed: BTreeSet<ContentHash>, whole: &BTreeSet<ContentHash>) -> Fetched {
    Fetched(
        needed
            .into_iter()
            .filter_map(|hash| {
                let got = match whole.contains(&hash) {
                    true => blobs.get(&hash).ok().map(|bytes| Got {
                        size: u64::try_from(bytes.len()).unwrap_or(u64::MAX),
                        whole: Some(bytes),
                    }),
                    false => blobs
                        .check(&hash)
                        .ok()
                        .map(|size| Got { size, whole: None }),
                }?;
                Some((hash, got))
            })
            .collect(),
    )
}

/// 发的时候从数据根里的 blob 读（施工 V-38）：核对的规矩照 [`Blobs::open_at`]，编码前核对过、没变过的直接读。
struct StoreBlobs(Blobs);

impl BlobSource for StoreBlobs {
    fn open(&self, blob: &ContentHash, offset: u64) -> io::Result<Box<dyn Read + Send>> {
        match self.0.open_at(blob, offset) {
            Ok(file) => Ok(Box::new(file)),
            Err(BlobError::Io(error)) => Err(error),
            Err(error) => Err(io::Error::other(error)),
        }
    }
}

/// 编码要的 blob 取不出来（`05-内核接口.md` 第七节）：分类「其他」，重试也没用。
pub(super) fn missing(hash: &ContentHash) -> CallError {
    CallError {
        class: ErrorClass::Unclassified,
        message: format!("编码要用的 blob {hash} 取不出来"),
        status: None,
        retry_after_ms: None,
    }
}

#[cfg(test)]
mod golden;
#[cfg(test)]
mod samples;
#[cfg(test)]
mod tests;
