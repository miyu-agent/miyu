//! `session/prompt` 的内容块凑成一句话和附件（设计 73 第五节第 3 条，`internal/blueprint/cli/acp.md`「说一句」）：文字照先后
//! 接起来；链接（`resource_link`）写进话里，本机文件写成路径（她要看就自己用工具读）；图片、嵌入的资源（`resource`）解开以后
//! 传给核心当附件（`blob.put`，大的分块 `blob.open`、`blob.write`、`blob.close`）。音频握手时没声明，来了不收。

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use serde_json::{Value, json};

use crate::rpc::Rpc;

/// 一块附件分块传时一块多大：核心的上限（`blob.md`「`blob.write`」一块最多 512 KiB）。比它小的一次传完。
const PIECE: usize = 512 * 1024;

/// 凑出来的一句话。
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct Said {
    /// 话。
    pub(crate) text: String,
    /// 要传的附件，照先后。
    pub(crate) files: Vec<File>,
}

/// 一件要传的附件。
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct File {
    /// 文件名（不带目录）。
    pub(crate) name: String,
    /// 编辑器说的类型；没说的由核心照内容认。
    pub(crate) media_type: Option<String>,
    /// 内容。
    pub(crate) bytes: Vec<u8>,
}

/// 读内容块。解不开的 base64 交回 `Err`（整句不收）。
pub(crate) fn read(prompt: &Value) -> Result<Said, ()> {
    let mut said = Said::default();
    let blocks = prompt.as_array().map_or(&[][..], Vec::as_slice);
    for block in blocks {
        match block["type"].as_str() {
            Some("text") => said
                .text
                .push_str(block["text"].as_str().unwrap_or_default()),
            Some("resource_link") => {
                let uri = block["uri"].as_str().unwrap_or_default();
                let place = local_path(uri).unwrap_or_else(|| uri.to_string());
                spaced(&mut said.text, &place);
            }
            Some("image") => {
                let bytes = decoded(&block["data"])?;
                let media_type = block["mimeType"].as_str().map(str::to_string);
                let extension = match media_type.as_deref().and_then(|t| t.strip_prefix("image/")) {
                    Some("jpeg") => "jpg",
                    Some(known @ ("png" | "gif" | "webp")) => known,
                    _ => "png",
                };
                let name = format!("image-{}.{extension}", said.files.len() + 1);
                said.files.push(File {
                    name,
                    media_type,
                    bytes,
                });
            }
            Some("resource") => {
                let resource = &block["resource"];
                let uri = resource["uri"].as_str().unwrap_or_default();
                let bytes = match resource["text"].as_str() {
                    Some(text) => text.as_bytes().to_vec(),
                    None => decoded(&resource["blob"])?,
                };
                said.files.push(File {
                    name: file_name(uri, said.files.len() + 1),
                    media_type: resource["mimeType"].as_str().map(str::to_string),
                    bytes,
                });
            }
            _ => {}
        }
    }
    Ok(said)
}

/// 解开一格 base64；不是字、解不开的是 `Err`。
fn decoded(data: &Value) -> Result<Vec<u8>, ()> {
    STANDARD.decode(data.as_str().ok_or(())?).map_err(|_| ())
}

/// 接一段进话里：和前后的字之间没有空白的补一个空格。
fn spaced(text: &mut String, piece: &str) {
    if text
        .chars()
        .last()
        .is_some_and(|last| !last.is_whitespace())
    {
        text.push(' ');
    }
    text.push_str(piece);
    text.push(' ');
}

/// 本机文件的地址（`file://`）写成路径：去掉主机名 `localhost`，解开 `%xx`；Windows 的盘符前面那个 `/` 去掉。别的地址没有。
pub(crate) fn local_path(uri: &str) -> Option<String> {
    let rest = uri.strip_prefix("file://")?;
    let rest = rest.strip_prefix("localhost").unwrap_or(rest);
    let bytes = rest.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        let hex = bytes
            .get(index + 1..index + 3)
            .and_then(|pair| std::str::from_utf8(pair).ok())
            .and_then(|pair| u8::from_str_radix(pair, 16).ok());
        match (bytes[index], hex) {
            (b'%', Some(byte)) => {
                out.push(byte);
                index += 3;
            }
            (byte, _) => {
                out.push(byte);
                index += 1;
            }
        }
    }
    let path = String::from_utf8(out).ok()?;
    let drive = path.as_bytes().get(1..3).is_some_and(|two| two[1] == b':');
    match cfg!(windows) && path.starts_with('/') && drive {
        true => Some(path[1..].to_string()),
        false => Some(path),
    }
}

/// 嵌入的资源的文件名：地址的最后一段（解开 `%xx`）；没有的叫 `file-<第几个>`。
fn file_name(uri: &str, nth: usize) -> String {
    let place = local_path(uri).unwrap_or_else(|| uri.to_string());
    let last = place
        .trim_end_matches(['/', '\\'])
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or_default();
    match last.is_empty() || last.contains(':') {
        true => format!("file-{nth}"),
        false => last.to_string(),
    }
}

/// 照先后把附件传给核心，交回 `session.send` 的 `attachments` 那几项。核心拒了的交回它的 `error`。
///
/// # Errors
///
/// 核心拒了（太大、读不懂）、断了（`error` 是 `null`）。
pub(crate) async fn attach(rpc: &mut Rpc, files: &[File]) -> Result<Vec<Value>, Value> {
    let mut attached = Vec::with_capacity(files.len());
    for file in files {
        let put = match file.bytes.len() <= PIECE {
            true => {
                let mut params = json!({"name": file.name, "data": STANDARD.encode(&file.bytes)});
                if let Some(media_type) = &file.media_type {
                    params["media_type"] = json!(media_type);
                }
                answered(rpc.call("blob.put", params).await)?
            }
            false => chunked(rpc, file).await?,
        };
        attached.push(
            json!({"blob": put["blob"], "name": put["name"], "media_type": put["media_type"]}),
        );
    }
    Ok(attached)
}

/// 大的分块传。
async fn chunked(rpc: &mut Rpc, file: &File) -> Result<Value, Value> {
    let mut params = json!({"name": file.name, "size": file.bytes.len()});
    if let Some(media_type) = &file.media_type {
        params["media_type"] = json!(media_type);
    }
    let opened = answered(rpc.call("blob.open", params).await)?;
    let upload = opened["upload"].clone();
    for (index, piece) in file.bytes.chunks(PIECE).enumerate() {
        let params =
            json!({"upload": upload, "offset": index * PIECE, "data": STANDARD.encode(piece)});
        answered(rpc.call("blob.write", params).await)?;
    }
    answered(rpc.call("blob.close", json!({"upload": upload})).await)
}

/// 核心的回应拆成做成的 `result`、被拒的 `error`；断了、写不出去的是 `null` 的 `error`。
pub(crate) fn answered(reply: std::io::Result<Option<Value>>) -> Result<Value, Value> {
    match reply {
        Ok(Some(reply)) => match reply.get("error") {
            None => Ok(reply["result"].clone()),
            Some(error) => Err(error.clone()),
        },
        _ => Err(Value::Null),
    }
}

#[cfg(test)]
mod tests;
