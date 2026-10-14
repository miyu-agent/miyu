//! `job.output`（施工 7-4 补，`docs/blueprint/protocol.md` 的 `job.output`）：头读一条后台命令到这时为止的输出，只要最后
//! 几行。读哪一份由执行器定，和她用 `jobs` 读的是同一份（`Handle::job_output`，`session/tools.md` 第 6 条第 3 款）；这里
//! 只取尾巴、量上限。
//!
//! 输出可能很长（一直开着的服务）：整份边读边数，不整份读进内存；一行再长，留在内存里的也只有它的末尾。头一直往下接的
//! 带 `after` 只要新长出来的那一段，带 `wait` 的没有新的就等（施工 V-6 上，[`after`]）。

use std::collections::VecDeque;
use std::io::{BufRead, BufReader, ErrorKind, Read};
use std::sync::Arc;

use serde::{Deserialize, Deserializer};
use serde_json::{Value, json};

use miyu_kernel::id::{JobId, SessionId};
use miyu_session::{Handle, Unreadable};
use miyu_tool::Output;

use crate::Core;
use crate::refusal::Refusal;

const TARGET: &str = "miyu::endpoint";

/// 不写 `tail` 时交最后几行。
const TAIL: u64 = 200;

/// `tail` 最多几行。
const MOST_LINES: u64 = 2000;

/// 一次回应里 `output` 最多多少字节（UTF-8）。一行最长 1 MiB（`wire::LINE_LIMIT`）；字写进 JSON，最坏一个字节变六个
/// （控制字符写成 `\u001b` 这样），回应的其余部分不到 1 KiB：6 × 128 KiB + 1 KiB 放得进一行（`protocol.md` 的
/// `job.output` 第 3 条）。
const CAP: usize = 128 * 1024;

/// `job.output` 的参数。
#[derive(Debug, Deserialize)]
pub(crate) struct Params {
    session: String,
    job: String,
    /// 要最后几行，不写是 [`TAIL`]。写 `null` 的、不是非负整数的读不成，是参数不对。
    #[serde(default, deserialize_with = "written")]
    tail: Option<u64>,
    /// 头已经有了前几个字节，只要后面的（施工 V-6 上）。和 `tail` 只能写一个。
    #[serde(default, deserialize_with = "written")]
    after: Option<u64>,
    /// 带 `after` 的：没有新的就等到有、或者命令结束了再答（施工 V-6 上）。
    #[serde(default)]
    wait: bool,
}

/// 写了的数：写 `null` 的读不成（不当没写）。
fn written<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<u64>, D::Error> {
    u64::deserialize(deserializer).map(Some)
}

/// 照哪种读。
enum Mode {
    /// 最后几行。
    Tail(usize),
    /// 第几个字节以后的，等不等。
    After { after: u64, wait: bool },
}

impl Params {
    /// 照哪种读：`tail`、`after` 都写了的，`wait` 没带 `after` 的，`tail` 不在 1 到 [`MOST_LINES`] 的，参数不对。
    fn mode(&self) -> Result<Mode, Refusal> {
        match (self.tail, self.after) {
            (Some(_), Some(_)) => Err(Refusal::BAD_PARAMS),
            (_, None) if self.wait => Err(Refusal::BAD_PARAMS),
            (_, Some(after)) => Ok(Mode::After {
                after,
                wait: self.wait,
            }),
            (tail, None) => {
                let tail = tail.unwrap_or(TAIL);
                if !(1..=MOST_LINES).contains(&tail) {
                    return Err(Refusal::BAD_PARAMS);
                }
                usize::try_from(tail)
                    .map(Mode::Tail)
                    .map_err(|_| Refusal::BAD_PARAMS)
            }
        }
    }
}

/// 读 `params` 说的那一条后台命令的输出，交回回应的 `result`。参数先查，不对的不找会话；再照 `job.stop` 查编号、找会话。
pub(crate) async fn read(core: &Arc<Core>, params: Params) -> Result<Value, Refusal> {
    let mode = params.mode()?;
    let session = SessionId::parse(&params.session).map_err(|_| Refusal::BAD_PARAMS)?;
    let job = JobId::parse(&params.job).map_err(|_| Refusal::BAD_PARAMS)?;
    let found = core.sessions.get(core, &session).await?;
    let handle = &found.handle;
    let want = match mode {
        Mode::Tail(want) => want,
        Mode::After { after, wait } => {
            return read_after(core, (&session, handle), &job, after, wait).await;
        }
    };
    let output = output_of(core, (&session, handle), &job).await?;
    let tail = match output.text {
        // 开不了的（没存下来）和空的一样，和 `jobs` 一样。
        None => Tail::default(),
        Some(text) => tokio::task::spawn_blocking(move || Tail::read(text, want))
            .await
            .map_err(|error| {
                tracing::error!(target: TARGET, error = %error, "job output panicked");
                Refusal::INTERNAL
            })?,
    };
    Ok(reply(tail, output.running))
}

/// 带 `after` 的（施工 V-6 上）：读 `after` 以后的；带 `wait` 的，没有新的、命令还在跑就等任务那一层叫一声再读。先拿着
/// 信号再读，读完到等之间长出来的也叫得醒。会话停了、信号断了的，照这时的答。
async fn read_after(
    core: &Core,
    (session, handle): (&SessionId, &Handle),
    job: &JobId,
    after: u64,
    wait: bool,
) -> Result<Value, Refusal> {
    let mut changes = match wait {
        true => Some(handle.job_changes().await.map_err(|_| Refusal::STOPPED)?),
        false => None,
    };
    loop {
        if let Some(changes) = changes.as_mut() {
            changes.borrow_and_update();
        }
        let output = output_of(core, (session, handle), job).await?;
        let running = output.running;
        let piece = match output.text {
            None => after::Piece {
                size: after,
                ..after::Piece::default()
            },
            Some(text) => tokio::task::spawn_blocking(move || after::read(text, after, running))
                .await
                .map_err(|error| {
                    tracing::error!(target: TARGET, error = %error, "job output panicked");
                    Refusal::INTERNAL
                })?,
        };
        let Some(waiting) = changes.as_mut() else {
            return Ok(piece.reply(running));
        };
        if !piece.output.is_empty() || !running || waiting.changed().await.is_err() {
            return Ok(piece.reply(running));
        }
    }
}

/// 读这一条的输出：没派过的 `unknown_job`，子代理 `not_a_command`；会话停了的从表里拿掉，`stopped`。
async fn output_of(
    core: &Core,
    (session, handle): (&SessionId, &Handle),
    job: &JobId,
) -> Result<Output, Refusal> {
    match handle.job_output(job.clone()).await {
        Ok(Ok(output)) => Ok(output),
        Ok(Err(Unreadable::Unknown)) => Err(Refusal::UNKNOWN_JOB),
        Ok(Err(Unreadable::Agent)) => Err(Refusal::NOT_A_COMMAND),
        Err(_) => {
            core.sessions.forget(session).await;
            Err(Refusal::STOPPED)
        }
    }
}

/// 回应的 `result`：格照名字的字母先后排（`serde_json` 的对象就是这样排的）。
fn reply(tail: Tail, running: bool) -> Value {
    json!({
        "lines": tail.lines,
        "output": tail.output,
        "running": running,
        "truncated": tail.truncated,
    })
}

/// 读到的尾巴。
#[derive(Debug, Default, PartialEq, Eq)]
struct Tail {
    /// 交的字：最后几行照原样接起来，每一行带着它的换行（最后一段没有换行的照样没有）。
    output: String,
    /// 一共几行。
    lines: u64,
    /// 前面还有没交的：去掉了前面的行，或者只交了一行的末尾。
    truncated: bool,
}

impl Tail {
    /// 从 `source` 读出最后 `want` 行（至少 1），一共不过 [`CAP`] 字节。一行照换行切，最后一段没有换行的也算一行，和 `jobs`
    /// 的数法一样（`tools/jobs.md` 第 3 条）；解不开的字节换成 `�`，也和它一样。读到一半读不下去的，读到多少算多少。
    fn read(source: impl Read, want: usize) -> Tail {
        let mut reader = BufReader::new(source);
        let mut kept = Kept::new(want);
        let mut line = Vec::new();
        let mut cut = false;
        loop {
            let chunk = match reader.fill_buf() {
                Ok([]) => break,
                Ok(chunk) => chunk,
                Err(error) if error.kind() == ErrorKind::Interrupted => continue,
                Err(_) => break,
            };
            let (take, ends) = match chunk.iter().position(|&byte| byte == b'\n') {
                Some(at) => (at + 1, true),
                None => (chunk.len(), false),
            };
            line.extend_from_slice(&chunk[..take]);
            reader.consume(take);
            // 一行太长：只留末尾，内存不跟着它涨。攒到两倍才挪，挪一次去掉一半。
            if line.len() > 2 * CAP {
                line.drain(..line.len() - CAP);
                cut = true;
            }
            if ends {
                kept.push(std::mem::take(&mut line), cut);
                cut = false;
            }
        }
        if !line.is_empty() {
            kept.push(line, cut);
        }
        kept.finish()
    }
}

/// 边读边留着的最后几行。
struct Kept {
    /// 最多留几行。
    want: usize,
    lines: VecDeque<String>,
    /// 留着的一共多少字节。
    bytes: usize,
    /// 一共读到几行。
    total: u64,
    /// 留着的只有一行，而且只留了它的末尾。
    cut: bool,
}

impl Kept {
    fn new(want: usize) -> Kept {
        Kept {
            want,
            lines: VecDeque::new(),
            bytes: 0,
            total: 0,
            cut: false,
        }
    }

    /// 读到一行（带着它的换行）；`cut` 是读的时候已经去掉了它的前面。
    fn push(&mut self, raw: Vec<u8>, cut: bool) {
        self.total += 1;
        let (text, cut) = fit(raw, cut);
        // 只留了末尾的一行，只在它是最后一行时交，而且只交它：前面接着别的行，看着就像它从那里开头（`protocol.md` 的
        // `job.output` 第 3 条）。它后面又来了一行，它整行去掉。
        if cut || self.cut {
            self.lines.clear();
            self.bytes = 0;
        }
        self.cut = cut;
        self.bytes += text.len();
        self.lines.push_back(text);
        // 刚来的这一行放得下（`fit`），`want` 至少是 1：去掉的只会是前面的。
        while self.lines.len() > self.want || self.bytes > CAP {
            match self.lines.pop_front() {
                Some(front) => self.bytes -= front.len(),
                None => break,
            }
        }
    }

    fn finish(self) -> Tail {
        let kept = u64::try_from(self.lines.len()).unwrap_or(u64::MAX);
        Tail {
            truncated: self.cut || kept < self.total,
            lines: self.total,
            output: self.lines.into_iter().collect(),
        }
    }
}

/// 一行写成字，最多 [`CAP`] 字节：超了的只留末尾，从一个字的开头起。交回的第二样是它的前面去掉过没有。
fn fit(mut raw: Vec<u8>, cut: bool) -> (String, bool) {
    // 读的时候去掉过前面的，开头可能是半个字：跳过接续字节，不让开头是 `�`。
    if cut {
        let partial = raw.iter().take_while(|&&byte| byte & 0xC0 == 0x80).count();
        raw.drain(..partial);
    }
    let mut text = String::from_utf8(raw)
        .unwrap_or_else(|error| String::from_utf8_lossy(error.as_bytes()).into_owned());
    if text.len() <= CAP {
        return (text, cut);
    }
    let mut at = text.len() - CAP;
    while !text.is_char_boundary(at) {
        at += 1;
    }
    text.drain(..at);
    (text, true)
}

mod after;
#[cfg(test)]
mod tests;
