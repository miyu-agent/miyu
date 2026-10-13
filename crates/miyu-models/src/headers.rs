//! 档案里另配的头（`docs/blueprint/models.md`「怎么走」第一条第 4 条、第八条第 1 条，施工 8-14）：值是模板，只认
//! `{session_digest}` 一个字段——26 位小写十六进制，照 opencode 的会话编号的样子：前 12 位是种子的时间照 opencode 的倒序编法，
//! 后 14 位是种子的 SHA-256 的十六进制前 14 位（施工 8-14 补：Zen 的免费档查前 12 位，乱码的一律 403）。种子是会话编号（同一个
//! 会话重启以后还是它，UUIDv7 里带着造它的时间），一次性调用是用途，`provider.test` 是固定的 `provider.test`；不是 UUIDv7 的
//! 照一个固定的时间。别的 `{…}` 读档案时就报错。
//!
//! 换好的头由路由挂到端点上（`miyu-session` 的 `route/choice.rs`、`route/probe.rs`），HTTP 执行器照端点另配的头发。

use sha2::{Digest, Sha256};

/// 模板里认得的那一个字段。
pub const SESSION_DIGEST: &str = "{session_digest}";

/// `provider.test` 用的种子：不属于哪个会话，固定一个。
pub const PROBE_SEED: &str = "provider.test";

/// 种子不是 UUIDv7 的照这个时间编前 12 位：2026-01-01T00:00:00Z 的毫秒数。固定的，同一个种子同一个值。
pub const FIXED_MS: u64 = 1_767_225_600_000;

/// 种子 `seed` 的摘要：前 12 位是时间（[`time_part`]），后 14 位是 SHA-256 的十六进制前 14 位。
pub fn digest(seed: &str) -> String {
    let hash = Sha256::digest(seed.as_bytes());
    let hex: String = hash.iter().map(|byte| format!("{byte:02x}")).collect();
    format!(
        "{}{}",
        time_part(uuid_v7_ms(seed).unwrap_or(FIXED_MS)),
        &hex[..14]
    )
}

/// 毫秒数 `ms` 照 opencode 的会话编号的前 12 位编：`ms × 4096` 按位取反，留低 48 位，写成 12 位十六进制（opencode 的
/// `Identifier.descending`，计数器是 0）。
pub fn time_part(ms: u64) -> String {
    let descending = !ms.wrapping_mul(0x1000) & 0xFFFF_FFFF_FFFF;
    format!("{descending:012x}")
}

/// `seed` 是 UUIDv7（`xxxxxxxx-xxxx-7xxx-…`）的，交回它前 48 位记的毫秒数；别的没有（前 12 位不是十六进制的解析不出来）。
fn uuid_v7_ms(seed: &str) -> Option<u64> {
    let hex: String = seed.chars().filter(|c| *c != '-').collect();
    match seed.len() == 36 && hex.len() == 32 && hex.as_bytes()[12] == b'7' {
        true => u64::from_str_radix(&hex[..12], 16).ok(),
        false => None,
    }
}

/// 模板 `template` 写得对不对：去掉认得的字段以后不能再有 `{`、`}`。不对的交回原话（不带值以外的东西）。
///
/// # Errors
///
/// 写了别的字段、括号没配对。
pub fn check(template: &str) -> Result<(), String> {
    match template.replace(SESSION_DIGEST, "").contains(['{', '}']) {
        true => Err(format!(
            "only {SESSION_DIGEST} can be filled in, found {template:?}"
        )),
        false => Ok(()),
    }
}

/// 照种子 `seed` 换好模板 `template`。
pub fn fill(template: &str, seed: &str) -> String {
    match template.contains(SESSION_DIGEST) {
        true => template.replace(SESSION_DIGEST, &digest(seed)),
        false => template.to_string(),
    }
}

#[cfg(test)]
mod tests;
