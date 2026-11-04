//! 原始包（施工 SE-1 上）：段原样、半行不要、`upto` 截在那一条、引用到的 blob 进包、不是 blob 的哈希记进 `absent`、已经有的
//! 不覆盖、blob 对不上整个失败且不留东西。

use std::fs;
use std::io::Read;

use zip::ZipArchive;

use super::*;
use crate::test_support::Scratch;

/// 一行事件：序号 `seq`，正文里放一个字 `text`。
fn line(seq: u64, text: &str) -> String {
    format!("{{\"seq\":{seq},\"type\":\"note\",\"body\":{{\"text\":\"{text}\"}}}}\n")
}

fn session(n: u8) -> SessionId {
    SessionId::parse(&format!("0192f3a0-0000-7000-8000-0000000000{n:02}")).expect("编号写得对")
}

fn about(main: &SessionId) -> About {
    About {
        main: main.clone(),
        miyu: "0.0.0".to_string(),
        exported: "2026-11-04T00:00:00Z".to_string(),
        transcript: None,
    }
}

/// 读回包里的一项。
fn entry(path: &Path, name: &str) -> Vec<u8> {
    let mut archive = ZipArchive::new(File::open(path).expect("包在")).expect("是 ZIP");
    let mut file = archive.by_name(name).expect("有这一项");
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).expect("读得出");
    bytes
}

fn names(path: &Path) -> Vec<String> {
    let archive = ZipArchive::new(File::open(path).expect("包在")).expect("是 ZIP");
    let mut names: Vec<String> = archive
        .file_names()
        .filter_map(Result::ok)
        .map(|name| name.into_owned())
        .collect();
    names.sort();
    names
}

fn leftovers(dir: &Path) -> Vec<String> {
    fs::read_dir(dir)
        .expect("读得了")
        .filter_map(Result::ok)
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.ends_with(".tmp"))
        .collect()
}

#[test]
fn segments_go_in_byte_for_byte_without_the_half_line_and_blobs_follow() {
    let scratch = Scratch::new();
    let blobs = Blobs::new(scratch.path().join("blobs"));
    let image = blobs.put(b"png bytes").expect("存得进");
    let not_a_blob = ContentHash::of(b"request bytes");
    let dir = scratch.path().join("sessions").join("main");
    fs::create_dir_all(&dir).expect("建得了");
    let first = format!("{}{}", line(1, "hi"), line(2, image.as_str()));
    fs::write(dir.join("000000000001.jsonl"), &first).expect("写得进");
    let second = format!("{}{{\"seq\":4,\"type\":\"no", line(3, not_a_blob.as_str()));
    fs::write(dir.join("000000000003.jsonl"), &second).expect("写得进");
    fs::write(dir.join("notes.txt"), "不是段").expect("写得进");
    let main = session(1);
    let out = scratch.path().join("out.zip");
    let parts = [Part {
        session: main.clone(),
        dir,
        upto: None,
    }];
    let written = write(&parts, &blobs, &about(&main), &out).expect("打得成");

    assert_eq!(
        entry(&out, &format!("sessions/{main}/000000000001.jsonl")),
        first.as_bytes()
    );
    assert_eq!(
        entry(&out, &format!("sessions/{main}/000000000003.jsonl")),
        line(3, not_a_blob.as_str()).as_bytes(),
        "没写完的半行不要"
    );
    assert_eq!(entry(&out, &format!("blobs/{}", image.hex())), b"png bytes");
    let manifest = &written.manifest;
    assert_eq!(manifest.format, FORMAT);
    assert_eq!(manifest.sessions[0].upto, Some(3));
    assert_eq!(manifest.sessions[0].segments.len(), 2);
    assert_eq!(
        manifest.sessions[0].segments[0].sha256,
        ContentHash::of(first.as_bytes())
    );
    assert_eq!(
        manifest.blobs,
        [ManifestBlob {
            hash: image,
            bytes: 9
        }]
    );
    assert_eq!(
        manifest.absent,
        [not_a_blob],
        "不是 blob 的哈希记下、不算错"
    );
    let read_back: Manifest = serde_json::from_slice(&entry(&out, MANIFEST)).expect("清单读得回");
    assert_eq!(&read_back, manifest);
    assert!(!names(&out).iter().any(|name| name.contains("notes.txt")));
    assert_eq!(written.bytes, fs::metadata(&out).expect("在").len());
    assert!(leftovers(scratch.path()).is_empty(), "临时文件收掉了");
}

#[test]
fn upto_stops_at_that_event_and_skips_later_segments() {
    let scratch = Scratch::new();
    let blobs = Blobs::new(scratch.path().join("blobs"));
    let later = blobs.put(b"after the fork").expect("存得进");
    let dir = scratch.path().join("sessions").join("source");
    fs::create_dir_all(&dir).expect("建得了");
    fs::write(
        dir.join("000000000001.jsonl"),
        format!(
            "{}{}{}",
            line(1, "a"),
            line(2, "b"),
            line(3, later.as_str())
        ),
    )
    .expect("写得进");
    fs::write(dir.join("000000000004.jsonl"), line(4, "d")).expect("写得进");
    let source = session(2);
    let out = scratch.path().join("fork.zip");
    let parts = [Part {
        session: source.clone(),
        dir,
        upto: Some(Seq::new(2).expect("不是 0")),
    }];
    let written = write(&parts, &blobs, &about(&source), &out).expect("打得成");

    assert_eq!(
        entry(&out, &format!("sessions/{source}/000000000001.jsonl")),
        format!("{}{}", line(1, "a"), line(2, "b")).as_bytes()
    );
    assert_eq!(
        written.manifest.sessions[0].segments.len(),
        1,
        "后面的段不要"
    );
    assert_eq!(written.manifest.sessions[0].upto, Some(2));
    assert!(
        written.manifest.blobs.is_empty(),
        "分叉点以后引用的 blob 不进包"
    );
}

#[test]
fn an_existing_file_is_not_overwritten() {
    let scratch = Scratch::new();
    let blobs = Blobs::new(scratch.path().join("blobs"));
    let dir = scratch.path().join("sessions").join("main");
    fs::create_dir_all(&dir).expect("建得了");
    fs::write(dir.join("000000000001.jsonl"), line(1, "hi")).expect("写得进");
    let out = scratch.path().join("taken.zip");
    fs::write(&out, "人家的").expect("写得进");
    let main = session(3);
    let parts = [Part {
        session: main.clone(),
        dir,
        upto: None,
    }];
    let got = write(&parts, &blobs, &about(&main), &out);
    assert!(matches!(got, Err(BundleError::Exists)), "{got:?}");
    assert_eq!(fs::read_to_string(&out).expect("在"), "人家的");
    assert!(leftovers(scratch.path()).is_empty());
}

#[test]
fn a_blob_that_does_not_match_its_name_fails_the_whole_bundle() {
    let scratch = Scratch::new();
    let blobs = Blobs::new(scratch.path().join("blobs"));
    let hash = blobs.put(b"original").expect("存得进");
    fs::write(blobs.path(&hash), b"tampered").expect("改得了");
    let dir = scratch.path().join("sessions").join("main");
    fs::create_dir_all(&dir).expect("建得了");
    fs::write(dir.join("000000000001.jsonl"), line(1, hash.as_str())).expect("写得进");
    let out = scratch.path().join("broken.zip");
    let main = session(4);
    let parts = [Part {
        session: main.clone(),
        dir,
        upto: None,
    }];
    let got = write(&parts, &blobs, &about(&main), &out);
    assert!(
        matches!(&got, Err(BundleError::BlobMismatch(bad)) if *bad == hash),
        "{got:?}"
    );
    assert!(out.symlink_metadata().is_err(), "失败了不留包");
    assert!(leftovers(scratch.path()).is_empty(), "也不留临时文件");
}

mod import;
mod transcript;
