//! 请求体的流（施工 V-38）：同一份片段清单造两次流，字节一样、和整份编的一样；送到一半停下的，哈希照样算完整；blob 读出来
//! 长短不对、读不了的，流出错；`Content-Length` 照片段清单算。

use std::collections::BTreeMap;
use std::future::poll_fn;
use std::io::Cursor;

use http_body::Body as _;
use miyu_drivers::{Encoded, Fragment};

use super::*;

mod measure;

/// 内存里的 blob：`short` 里的读出来少一个字节。
#[derive(Default)]
struct Memory {
    blobs: BTreeMap<ContentHash, Vec<u8>>,
    short: Option<ContentHash>,
}

impl BlobSource for Memory {
    fn open(&self, blob: &ContentHash, offset: u64) -> io::Result<Box<dyn Read + Send>> {
        let mut bytes = self
            .blobs
            .get(blob)
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "没有"))?
            .clone();
        if self.short.as_ref() == Some(blob) {
            bytes.pop();
        }
        let offset = usize::try_from(offset).unwrap().min(bytes.len());
        Ok(Box::new(Cursor::new(bytes[offset..].to_vec())))
    }
}

/// `sample` 里那张大图多少个字节：编出来比四块 1 MiB 少一点，加上前后的字，一条流正好交 5 块。
const BIG: usize = 3_000_000;

fn picture(len: usize, seed: u32) -> Vec<u8> {
    (0..len)
        .map(|i| ((i as u32).wrapping_mul(2_654_435_761).wrapping_add(seed) >> 13) as u8)
        .collect()
}

/// 字、一张 3 MB 的图（编出来 4 MB，攒成好几块，施工 PF-1）、字、一张 4 个字节的图、字。
fn sample() -> (Fragments, Memory) {
    let big = picture(BIG, 1);
    let small = picture(4, 2);
    let (big_hash, small_hash) = (ContentHash::of(&big), ContentHash::of(&small));
    let mut memory = Memory::default();
    memory.blobs.insert(big_hash.clone(), big);
    memory.blobs.insert(small_hash.clone(), small);
    let text = |text: &str| Fragment::Text(text.as_bytes().to_vec());
    let parts = vec![
        text("{\"a\":\"data:image/png;base64,"),
        Fragment::Base64 {
            blob: big_hash,
            size: u64::try_from(BIG).unwrap(),
        },
        text("\",\"b\":\""),
        Fragment::Base64 {
            blob: small_hash,
            size: 4,
        },
        text("\"}"),
    ];
    (parts.into_iter().collect(), memory)
}

/// 整份的字节：驱动那边拼的。
fn whole(fragments: &Fragments, memory: &Memory) -> Vec<u8> {
    let encoded = Encoded {
        body: fragments.clone(),
        messages: Vec::new(),
        path: String::new(),
    };
    encoded.to_bytes(&memory.blobs).unwrap().body
}

async fn next(stream: &mut Stream) -> Option<Result<Bytes, io::Error>> {
    let frame = poll_fn(|cx| Pin::new(&mut *stream).poll_frame(cx)).await?;
    Some(frame.map(|frame| frame.into_data().unwrap()))
}

async fn drain(stream: &mut Stream) -> Result<Vec<u8>, io::Error> {
    let mut out = Vec::new();
    while let Some(chunk) = next(stream).await {
        out.extend_from_slice(&chunk?);
    }
    Ok(out)
}

#[tokio::test]
async fn two_streams_from_the_same_fragments_give_the_same_bytes() {
    let (fragments, memory) = sample();
    let expected = whole(&fragments, &memory);
    let replay = Arc::new(Replay::new(Body::new(fragments, Arc::new(memory))));
    let mut first = replay.stream();
    assert_eq!(
        first.size_hint().exact(),
        Some(u64::try_from(expected.len()).unwrap()),
        "Content-Length 照片段清单算，和整份的一样长"
    );
    let first = drain(&mut first).await.unwrap();
    let second = drain(&mut replay.stream()).await.unwrap();
    assert!(first == expected, "第一条流和整份编的一样");
    assert!(second == expected, "重放的一样");
    assert_eq!(replay.hash().await, ContentHash::of(&expected));
}

#[tokio::test]
async fn a_stream_stopped_halfway_still_gets_the_whole_hash() {
    let (fragments, memory) = sample();
    let expected = whole(&fragments, &memory);
    let (fragments_again, memory_again) = sample();
    for stop_after in [0, 1, 2, 3] {
        let replay = Arc::new(Replay::new(Body::new(
            fragments.clone(),
            Arc::new(Memory {
                blobs: memory.blobs.clone(),
                short: None,
            }),
        )));
        let mut stream = replay.stream();
        for _ in 0..stop_after {
            next(&mut stream).await.unwrap().unwrap();
        }
        drop(stream);
        assert_eq!(
            replay.hash().await,
            ContentHash::of(&expected),
            "送了 {stop_after} 块就停：剩下的生成出来算完"
        );
    }
    // 两条流各送到不同的地方：重放的那一条从算过的地方接着算。
    let replay = Arc::new(Replay::new(Body::new(
        fragments_again,
        Arc::new(memory_again),
    )));
    let mut first = replay.stream();
    for _ in 0..3 {
        next(&mut first).await.unwrap().unwrap();
    }
    let mut second = replay.stream();
    for _ in 0..5 {
        next(&mut second).await.unwrap().unwrap();
    }
    drop(first);
    assert_eq!(replay.hash().await, ContentHash::of(&expected));
    assert!(drain(&mut second).await.is_ok(), "还在送的那一条照样送完");
}

#[tokio::test]
async fn a_blob_that_reads_short_or_not_at_all_breaks_the_stream() {
    let (fragments, memory) = sample();
    let big = match &fragments.parts()[1] {
        Fragment::Base64 { blob, .. } => blob.clone(),
        Fragment::Text(_) => unreachable!(),
    };
    let short = Memory {
        blobs: memory.blobs.clone(),
        short: Some(big.clone()),
    };
    let replay = Arc::new(Replay::new(Body::new(fragments.clone(), Arc::new(short))));
    let error = drain(&mut replay.stream()).await.unwrap_err();
    assert!(error.to_string().contains("比编码时量的短"), "{error}");
    let mut gone = memory;
    gone.blobs.remove(&big);
    let replay = Arc::new(Replay::new(Body::new(fragments.clone(), Arc::new(gone))));
    let error = drain(&mut replay.stream()).await.unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::NotFound);
    // 没有读 blob 的地方的（只该给没有 blob 的请求体）。
    let replay = Arc::new(Replay::new(Body::from(fragments)));
    assert!(drain(&mut replay.stream()).await.is_err());
}

#[tokio::test]
async fn whole_bytes_are_one_piece() {
    let replay = Arc::new(Replay::new(Body::from(b"{}".to_vec())));
    assert_eq!(drain(&mut replay.stream()).await.unwrap(), b"{}");
    assert_eq!(replay.hash().await, ContentHash::of(b"{}"));
}

/// 夹着很多小图的请求体（施工 PF-1）：交给 HTTP 客户端的块数照请求体多大算，不照有几张图算。原来一张图一个阻塞任务、48 KiB
/// 一块，2 MB 的请求体要交七八十块，CPU 被抢时每一块都要等阻塞线程、再排一次队，压测里收一个请求体从 4 ms 涨到秒级。
#[tokio::test]
async fn many_small_pictures_go_out_in_a_few_big_chunks() {
    let (fragments, memory) = measure::bench_body();
    let expected = whole(&fragments, &memory);
    let replay = Arc::new(Replay::new(Body::new(fragments, Arc::new(memory))));
    let mut stream = replay.stream();
    let (mut out, mut frames) = (Vec::new(), 0usize);
    while let Some(chunk) = next(&mut stream).await {
        out.extend_from_slice(&chunk.unwrap());
        frames += 1;
    }
    assert!(out == expected, "和整份编的一个字节都不差");
    assert_eq!(replay.hash().await, ContentHash::of(&expected));
    // 第一张图前面的两段字照原样交，从第一张图起攒成 1 MiB 一块。
    let most = expected.len().div_ceil(batch::BATCH) + 2;
    assert!(frames <= most, "交了 {frames} 块，最多该是 {most} 块");
}

/// 图后面跟着比一块还长的一段字（施工 PF-1）：那一段不拷、照原样交，先交攒着的；字节、哈希和整份编的一样。
#[tokio::test]
async fn a_text_longer_than_a_batch_after_a_picture_goes_out_as_is() {
    let small = picture(10, 3);
    let hash = ContentHash::of(&small);
    let mut memory = Memory::default();
    memory.blobs.insert(hash.clone(), small);
    let long = vec![b'y'; batch::BATCH + 7];
    let parts = vec![
        Fragment::Base64 {
            blob: hash,
            size: 10,
        },
        Fragment::Text(long.clone()),
        Fragment::Text(b"}".to_vec()),
    ];
    let fragments: Fragments = parts.into_iter().collect();
    let expected = whole(&fragments, &memory);
    let replay = Arc::new(Replay::new(Body::new(fragments, Arc::new(memory))));
    let mut stream = replay.stream();
    let mut chunks = Vec::new();
    while let Some(chunk) = next(&mut stream).await {
        chunks.push(chunk.unwrap());
    }
    let lengths: Vec<usize> = chunks.iter().map(Bytes::len).collect();
    assert_eq!(lengths, [16, long.len(), 1], "图、长的那段、结尾各一块");
    assert!(chunks.concat() == expected);
    assert_eq!(replay.hash().await, ContentHash::of(&expected));
}
