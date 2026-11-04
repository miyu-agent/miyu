//! 编好的请求体是一串片段（施工 V-38）：编好的字，和「这里是某个 blob 整份的 base64」。照先后拼起来就是发出去的字节，和以前
//! 整份编成一个 `Vec<u8>` 的一个字节都不差。
//!
//! 为什么拆：一张图原来要在内存里同时有原图、data URL 的字符串、编好的请求体、交给 HTTP 客户端的那一份，一直放到模型回完。
//! 拆开以后编码时图片只记 blob 和大小，发的时候 HTTP 执行器才一小块一小块地从 blob 读出来、编成 base64 送出去，送完就扔；
//! 片段清单本身只有字和 blob 的引用，留着它可以从头再造一遍同样的字节（跳转、对方没处理的重发）。
//!
//! 怎么拆：线上的结构里要内嵌 blob 的那一格是 [`Inline`]，照常经 serde 写成 JSON。它写的时候不出字，交出一个只有
//! [`Builder::json`] 的写法认得的记号；那个写法在那里写下开头的引号和前缀（`data:<类型>;base64,`，照 JSON 转义）、一个
//! `0xFF`、结尾的引号，记下是哪个 blob。`0xFF` 在 UTF-8 里不会出现，serde_json 写出来的又都是合法的 UTF-8，写完照它切开：
//! 前面的字收成一段，那里记一段 base64。base64 的字母表里没有要转义的字，所以拆开写和整份写一个字节都不差。一张图都没有的
//! 不切，和原来直接写进 `Vec<u8>` 一样快。

use std::io;
use std::mem;

use miyu_kernel::id::ContentHash;
use serde::{Serialize, Serializer};

use crate::{BlobBytes, EncodeError, Encoded, base64};

/// 编好的请求体：一串片段，照先后拼起来就是发出去的字节。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Fragments {
    parts: Vec<Fragment>,
    len: u64,
}

/// 请求体里的一段。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Fragment {
    /// 编好的字，照原样发。
    Text(Vec<u8>),
    /// 一个 blob 整份的 base64：标准字母表，末尾补 `=`，不换行（[`base64::encode`]）。`size` 是 blob 的字节数，编码时
    /// 照它算长短，发的时候读出来的不是这么多就是出错。
    Base64 {
        /// 哪一个 blob。
        blob: ContentHash,
        /// blob 有多少个字节。
        size: u64,
    },
}

impl Fragment {
    /// 这一段发出去有多少个字节。
    pub fn len(&self) -> u64 {
        match self {
            Fragment::Text(text) => u64::try_from(text.len()).unwrap_or(u64::MAX),
            Fragment::Base64 { size, .. } => base64::encoded_len(*size),
        }
    }

    /// 这一段一个字节都没有。
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Fragments {
    /// 拼起来一共多少个字节：发出去的 `Content-Length`。
    pub fn len(&self) -> u64 {
        self.len
    }

    /// 一个字节都没有。
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// 编好的字一共多少个字节（base64 那几段不算）：拼进请求体时照它先留地方（施工 V-42）。
    pub(crate) fn text_len(&self) -> usize {
        self.parts
            .iter()
            .map(|part| match part {
                Fragment::Text(text) => text.len(),
                Fragment::Base64 { .. } => 0,
            })
            .sum()
    }

    /// 各段，照先后。
    pub fn parts(&self) -> &[Fragment] {
        &self.parts
    }

    /// 交出各段，照先后。
    pub fn into_parts(self) -> Vec<Fragment> {
        self.parts
    }

    /// 拼成整份的字节：base64 那几段照 `blobs` 里整份的字节编（测试、对照用；发请求不走这里）。
    ///
    /// # Errors
    ///
    /// 要用的 blob 没交进来，或者交进来的长短和编码时记的不一样。
    pub fn to_bytes(&self, blobs: &dyn BlobBytes) -> Result<Vec<u8>, EncodeError> {
        let mut body = Vec::with_capacity(usize::try_from(self.len).unwrap_or(0));
        for part in &self.parts {
            match part {
                Fragment::Text(text) => body.extend_from_slice(text),
                Fragment::Base64 { blob, size } => {
                    let bytes = blobs
                        .bytes(blob)
                        .filter(|bytes| u64::try_from(bytes.len()).ok() == Some(*size))
                        .ok_or_else(|| EncodeError::MissingBlob(blob.clone()))?;
                    body.extend_from_slice(base64::encode(bytes).as_bytes());
                }
            }
        }
        Ok(body)
    }
}

/// 整份编好的字：一段。
impl From<Vec<u8>> for Fragments {
    fn from(text: Vec<u8>) -> Fragments {
        let mut builder = Builder::default();
        builder.extend_from_slice(&text);
        builder.finish()
    }
}

/// 照先后的各段（测试拼请求体用）：相邻的字不合并。
impl FromIterator<Fragment> for Fragments {
    fn from_iter<I: IntoIterator<Item = Fragment>>(parts: I) -> Fragments {
        let parts: Vec<Fragment> = parts.into_iter().collect();
        let len = parts
            .iter()
            .fold(0u64, |len, part| len.saturating_add(part.len()));
        Fragments { parts, len }
    }
}

impl Encoded<Fragments> {
    /// 拼成整份的字节（[`Fragments::to_bytes`]），每条消息的位置、路径照旧。
    ///
    /// # Errors
    ///
    /// 同 [`Fragments::to_bytes`]。
    pub fn to_bytes(self, blobs: &dyn BlobBytes) -> Result<Encoded, EncodeError> {
        Ok(Encoded {
            body: self.body.to_bytes(blobs)?,
            messages: self.messages,
            path: self.path,
        })
    }
}

/// 一个值单独写成一截（施工 V-42：留着的那几截照它编），写法同 [`Builder::json`]。
pub(crate) fn piece(value: &(impl Serialize + ?Sized)) -> Fragments {
    let mut builder = Builder::default();
    builder.json(value);
    builder.finish()
}

/// 一边写一边拆成片段：字照常接在后面，碰到 [`Inline`] 收成一段、记一段 base64。各驱动编码时用它代替 `Vec<u8>`。
#[derive(Debug, Default)]
pub(crate) struct Builder {
    /// 收好的片段。
    parts: Vec<Fragment>,
    /// 收好的片段一共多少个字节。
    done: u64,
    /// 还没收的字。
    text: Vec<u8>,
}

impl Builder {
    /// 接上这些字节。
    pub(crate) fn extend_from_slice(&mut self, bytes: &[u8]) {
        self.text.extend_from_slice(bytes);
    }

    /// 接上一个字节。
    pub(crate) fn push(&mut self, byte: u8) {
        self.text.push(byte);
    }

    /// 去掉最后一个字节（anthropic 的打点：接在一块的最后一格后面，先去掉那一块结尾的 `}`）。最后一段是 base64 的没有可去的，
    /// 交回空的：JSON 的值都以字收尾，走不到。
    pub(crate) fn pop(&mut self) -> Option<u8> {
        self.text.pop()
    }

    /// 到这里一共写了多少个字节：每条消息的位置照它记。
    pub(crate) fn len(&self) -> usize {
        usize::try_from(self.done)
            .unwrap_or(usize::MAX)
            .saturating_add(self.text.len())
    }

    /// 写成紧凑的 JSON 接在后面：和 `serde_json::to_writer` 写的一个字节都不差，只是 [`Inline`] 那几格拆成片段。
    ///
    /// # Panics
    ///
    /// 实际不会 panic：线上的消息里没有写不成 JSON 的东西。
    pub(crate) fn json(&mut self, value: &(impl Serialize + ?Sized)) {
        let start = self.text.len();
        let mut inlines = Vec::new();
        value
            .serialize(&mut serde_json::Serializer::with_formatter(
                &mut self.text,
                Inliner(&mut inlines),
            ))
            .expect("线上的消息里没有写不成 JSON 的东西");
        if inlines.is_empty() {
            return;
        }
        // 照记号切开：第几个记号就是第几个内嵌的 blob。
        let written = self.text.split_off(start);
        let mut pieces = written.split(|byte| *byte == MARK);
        if let Some(first) = pieces.next() {
            self.text.extend_from_slice(first);
        }
        for (inline, piece) in inlines.into_iter().zip(pieces) {
            self.inline(inline.blob, inline.size);
            self.text.extend_from_slice(piece);
        }
    }

    /// 接上一截编好的（施工 V-42：留着的历史消息）：字照样接在后面，base64 那几段照样记，和当初直接写进来的一个字节都不差。
    pub(crate) fn append(&mut self, fragments: &Fragments) {
        for part in &fragments.parts {
            match part {
                Fragment::Text(text) => self.text.extend_from_slice(text),
                Fragment::Base64 { blob, size } => self.inline(blob.clone(), *size),
            }
        }
    }

    /// 先留出 `more` 个字节的地方（施工 V-42：照留着的那几截的长短一次留够，不一倍一倍地搬）。
    pub(crate) fn reserve(&mut self, more: usize) {
        self.text.reserve(more);
    }

    /// 写好了：收起最后一段。
    pub(crate) fn finish(mut self) -> Fragments {
        self.flush();
        Fragments {
            parts: self.parts,
            len: self.done,
        }
    }

    /// 前面攒的字收成一段，再记一段 base64。
    fn inline(&mut self, blob: ContentHash, size: u64) {
        self.flush();
        let part = Fragment::Base64 { blob, size };
        self.done = self.done.saturating_add(part.len());
        self.parts.push(part);
    }

    /// 攒的字收成一段；多申请的那一截还给分配器（大的一段是单独映射的，缩起来不用搬）。
    fn flush(&mut self) {
        if self.text.is_empty() {
            return;
        }
        let mut text = mem::take(&mut self.text);
        text.shrink_to_fit();
        self.done = self
            .done
            .saturating_add(u64::try_from(text.len()).unwrap_or(u64::MAX));
        self.parts.push(Fragment::Text(text));
    }
}

/// 线上的结构里内嵌一个 blob 的那一格：写成 JSON 的字符串 `"<前缀><blob 整份的 base64>"`，base64 不在编码时编。只有
/// [`Builder::json`] 认得它：别的写法写出来是一串数，不是这一格该有的样子。
#[derive(Debug, Clone)]
pub(crate) struct Inline {
    /// base64 前面的字：data URL 的 `data:<类型>;base64,`，anthropic 的是空的。照 JSON 转义写。
    prefix: String,
    /// 哪一个 blob。
    blob: ContentHash,
    /// blob 有多少个字节。
    size: u64,
}

impl Inline {
    /// 只有 base64（anthropic 的 `source.data`）。
    pub(crate) fn bare(blob: &ContentHash, size: u64) -> Inline {
        Inline {
            prefix: String::new(),
            blob: blob.clone(),
            size,
        }
    }

    /// `data:<类型>;base64,<内容>`。
    pub(crate) fn data_url(media_type: &str, blob: &ContentHash, size: u64) -> Inline {
        Inline {
            prefix: format!("data:{media_type};base64,"),
            blob: blob.clone(),
            size,
        }
    }

    /// 交给写法的记号：大小（8 个字节，小端）、哈希的长短（8 个字节，小端）、哈希、前缀。
    fn token(&self) -> Vec<u8> {
        let hash = self.blob.as_str().as_bytes();
        let length = u64::try_from(hash.len()).unwrap_or(u64::MAX);
        let mut token = Vec::with_capacity(16 + hash.len() + self.prefix.len());
        token.extend_from_slice(&self.size.to_le_bytes());
        token.extend_from_slice(&length.to_le_bytes());
        token.extend_from_slice(hash);
        token.extend_from_slice(self.prefix.as_bytes());
        token
    }

    /// 从记号读回来；不是 [`Inline::token`] 写的交回空的。
    fn from_token(token: &[u8]) -> Option<Inline> {
        let (size, rest) = token.split_first_chunk::<8>()?;
        let (length, rest) = rest.split_first_chunk::<8>()?;
        let length = usize::try_from(u64::from_le_bytes(*length)).ok()?;
        let (hash, prefix) = rest.split_at_checked(length)?;
        Some(Inline {
            prefix: String::from_utf8(prefix.to_vec()).ok()?,
            blob: ContentHash::parse(std::str::from_utf8(hash).ok()?).ok()?,
            size: u64::from_le_bytes(*size),
        })
    }
}

impl Serialize for Inline {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_bytes(&self.token())
    }
}

/// 内嵌的 blob 在写出来的字里的记号：UTF-8 里没有这个字节。
const MARK: u8 = 0xFF;

/// [`Builder::json`] 的写法：别的照 serde_json 紧凑的写法（默认的那几样），只有 [`Inline`] 交来的记号写成
/// `"<前缀>` + [`MARK`] + `"`，照先后记下是哪个 blob。线上的结构里没有别的一串字节（serde 的 `serialize_bytes`），只有它走这里。
struct Inliner<'a>(&'a mut Vec<Inline>);

impl serde_json::ser::Formatter for Inliner<'_> {
    fn write_byte_array<W>(&mut self, writer: &mut W, value: &[u8]) -> io::Result<()>
    where
        W: ?Sized + io::Write,
    {
        let inline = Inline::from_token(value)
            .ok_or_else(|| io::Error::other("一串字节不是内嵌 blob 的记号"))?;
        // 开头的引号和前缀，照 JSON 转义：整串写成 JSON 字符串，去掉结尾的引号。
        let quoted = serde_json::to_vec(&inline.prefix).map_err(io::Error::other)?;
        let (_, open) = quoted
            .split_last()
            .ok_or_else(|| io::Error::other("JSON 字符串至少有两个引号"))?;
        writer.write_all(open)?;
        writer.write_all(&[MARK, b'"'])?;
        self.0.push(inline);
        Ok(())
    }
}

#[cfg(test)]
mod tests;
