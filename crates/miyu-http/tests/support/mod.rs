//! 几个测试共用的：一份请求字节、一个驱动、驱动的流的样本；带一张图的请求体和内存里的 blob（施工 V-38）；HTTP/2 的假服务器
//! （`h2c.rs`）。HTTP/1.1 的假服务器在 `miyu_http::testkit`。

#![allow(dead_code, reason = "几个测试各用其中一部分")]

pub mod h2c;

use std::collections::BTreeMap;
use std::io::{self, Cursor, Read};
use std::path::PathBuf;
use std::sync::Arc;

use miyu_drivers::openai_chat::Compat;
use miyu_drivers::{DriverTextSources, DriverTexts, Encoded, Fragment, Fragments, OpenAiChat};
use miyu_http::{BlobSource, Body};
use miyu_kernel::id::ContentHash;

/// 一份请求字节：内容无所谓，查的是一字不差地发出去。里面的「你好」也拿来查日志里没有正文。
pub const BODY: &[u8] =
    r#"{"model":"deepseek-v4","messages":[{"role":"user","content":"你好"}],"stream":true}"#
        .as_bytes();

/// OpenAI 兼容的驱动，文字都是占位。
pub fn driver() -> OpenAiChat {
    let texts = DriverTexts::new(DriverTextSources {
        image_omitted: "image\n",
        file_omitted: "file {name} {media_type}\n",
        no_output: "nothing\n",
        tool_attachments: "attachments\n",
        tool_attachments_only: "only\n",
        text_file: None,
        image_name: None,
        image_description: None,
        attached_path: None,
        open_path: None,
        path_only: None,
        hosted_tool: None,
    })
    .expect("占位用得了");
    OpenAiChat::new(Compat::default(), texts)
}

/// 驱动的流的样本。
pub fn sample(name: &str) -> Vec<u8> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/designs/samples/drivers/openai-chat/streams")
        .join(format!("{name}.sse"));
    std::fs::read(&path).unwrap_or_else(|e| panic!("读不了 {}：{e}", path.display()))
}

/// 内存里的 blob（施工 V-38）。
pub struct Memory(pub BTreeMap<ContentHash, Vec<u8>>);

impl BlobSource for Memory {
    fn open(&self, blob: &ContentHash, offset: u64) -> io::Result<Box<dyn Read + Send>> {
        let bytes = self
            .0
            .get(blob)
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "没有这个 blob"))?;
        let offset = usize::try_from(offset).expect("不大").min(bytes.len());
        Ok(Box::new(Cursor::new(bytes[offset..].to_vec())))
    }
}

/// 带一张 200 KB 的图的请求体（base64 有好几块，过了 h2 默认的 64 KiB 窗口），和它整份的字节。
pub fn with_picture() -> (Body, Vec<u8>) {
    let picture: Vec<u8> = (0..200_000u32)
        .map(|i| (i.wrapping_mul(2_654_435_761) >> 13) as u8)
        .collect();
    let blob = ContentHash::of(&picture);
    let fragments: Fragments = [
        Fragment::Text(br#"{"model":"m","messages":[{"role":"user","content":[{"type":"image_url","image_url":{"url":"data:image/png;base64,"#.to_vec()),
        Fragment::Base64 {
            blob: blob.clone(),
            size: 200_000,
        },
        Fragment::Text(br#""}}]}],"stream":true}"#.to_vec()),
    ]
    .into_iter()
    .collect();
    let blobs = BTreeMap::from([(blob, picture)]);
    let whole = Encoded {
        body: fragments.clone(),
        messages: Vec::new(),
        path: String::new(),
    }
    .to_bytes(&blobs)
    .expect("拼得起来")
    .body;
    (Body::new(fragments, Arc::new(Memory(blobs))), whole)
}
