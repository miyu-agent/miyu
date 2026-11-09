//! `miyu pkg page <编号>`（施工 BP-2，`package-pages.md`「不靠网页打开软件后台」第 1、2 条）：不用装网页，用浏览器打开一个包的
//! 后台页。后台页是程序自己起的服务的（`package.list` 带 `page_server`）：问它的 `status` 拿 `web` 端口，要一张 `login.ticket`，开
//! `http://127.0.0.1:<端口>/#miyu=<base64url(JSON {v: 1, ticket, lang})>`（不带颜色、深浅，那边照自己记的或者系统的）。别的交给
//! 核心 `package.page`：目录里的后台页由核心按需起的服务给；没装、没有后台页的照核心的原话说。网址总印出来，浏览器没开成的
//! 照样能复制；`--print` 只印不开。

use std::io::Write;

use serde_json::{Value, json};

use super::ask;
use crate::diagnostic;
use crate::exit;
use crate::language::Language;
use crate::rpc::Rpc;
use crate::shown::say;

/// 打开 `package`（`print` 的只印）。
pub(super) async fn open(
    rpc: &mut Rpc,
    language: &Language,
    (package, print): (&str, bool),
    (out, err): (&mut dyn Write, &mut dyn Write),
) -> Result<(), u8> {
    let listed = ask(rpc, "package.list", json!({}), language, err).await?;
    let served = listed["packages"]
        .as_array()
        .into_iter()
        .flatten()
        .any(|item| item["package"] == package && item["page_server"] == true);
    let url = match served {
        true => site(rpc, language, package, err).await?,
        false => {
            let params = json!({"package": package});
            let opened = ask(rpc, "package.page", params, language, err).await?;
            opened["url"].as_str().unwrap_or_default().to_string()
        }
    };
    say(out, &url);
    if !print && !crate::mcp::opened(&url) {
        diagnostic::error(err, *language, &language.browser_not_opened());
    }
    Ok(())
}

/// 程序自己起的服务的网址：程序没在跑、没写 `web` 的说一句。
async fn site(
    rpc: &mut Rpc,
    language: &Language,
    package: &str,
    err: &mut dyn Write,
) -> Result<String, u8> {
    let status = crate::link::answer(
        rpc,
        "package.call",
        json!({"package": package, "method": "status"}),
        language,
        err,
    )
    .await?;
    let Some(port) = status
        .ok()
        .and_then(|status| status["web"].as_u64())
        .and_then(|port| u16::try_from(port).ok())
        .filter(|port| *port > 0)
    else {
        diagnostic::error(err, *language, &language.page_not_running(package));
        return Err(exit::ERROR);
    };
    let ticket = ask(rpc, "login.ticket", json!({}), language, err).await?;
    let ticket = ticket["ticket"].as_str().unwrap_or_default();
    Ok(site_url(port, ticket, language.code()))
}

/// 照网页 `external.js` 的写法拼：片段是 `miyu=` 接 JSON 的 base64url（不带 `=`）。
fn site_url(port: u16, ticket: &str, lang: &str) -> String {
    let handed: Value = json!({"v": 1, "ticket": ticket, "lang": lang});
    format!(
        "http://127.0.0.1:{port}/#miyu={}",
        base64url(handed.to_string().as_bytes())
    )
}

/// base64url，不带 `=`。
fn base64url(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = chunk
            .iter()
            .enumerate()
            .fold(0u32, |n, (at, byte)| n | u32::from(*byte) << (16 - 8 * at));
        let take = chunk.len() + 1;
        for at in 0..take {
            let index = (n >> (18 - 6 * at)) & 63;
            out.push(char::from(ALPHABET[index as usize]));
        }
    }
    out
}

#[cfg(test)]
mod tests;
