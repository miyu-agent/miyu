//! base64 编码（RFC 4648 第四节）：标准字母表，末尾补 `=`，不换行。图片、文件写成 data URL 时用，只要编码这一半。
//!
//! 施工 V-38 换成 `base64` 这个库（原来自己一个字一个字地拼，有图的请求 CPU 里它占两成）：写出来的一个字节都不差，
//! 测试守着 RFC 的测试值和自己原来的写法逐字一样。发请求体时一小块一小块地编（[`CHUNK`]），块的长短是 3 的倍数，
//! 中间的块不补 `=`，接起来和整份编的一样。

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;

/// 发请求体时一次从 blob 读多少个字节再编：48 KiB，是 3 的倍数，编出来正好 64 KiB。
pub const CHUNK: usize = 48 * 1024;

/// 编码成 base64：每 3 个字节写成 4 个字符，最后不满 3 个字节的，照缺几个补几个 `=`。
pub fn encode(bytes: &[u8]) -> String {
    STANDARD.encode(bytes)
}

/// `size` 个字节编出来有多少个字符。
pub fn encoded_len(size: u64) -> u64 {
    size.div_ceil(3).saturating_mul(4)
}

#[cfg(test)]
mod tests;
