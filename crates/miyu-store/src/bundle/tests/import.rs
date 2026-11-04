//! 导入原始包（施工 SE-1 下）：导出再导入一个字节不差、带上标记；重导是空操作；存档的再导出、再导入还是存档；坏了的（哈希不对、
//! 序号不连、引用的 blob 不在包里、格式太新）什么都不留；同编号内容不一样的拒。

use std::io::Write;

use zip::write::SimpleFileOptions;

use super::*;

/// 一个数据根里的会话目录和 blob 库。
struct Side {
    sessions: PathBuf,
    blobs: Blobs,
}

fn side(scratch: &Scratch, name: &str) -> Side {
    let root = scratch.path().join(name);
    fs::create_dir_all(&root).expect("建得了");
    Side {
        sessions: root.join("sessions"),
        blobs: Blobs::new(root.join("blobs")),
    }
}

fn marker(archive: bool) -> Marker {
    Marker {
        archive,
        miyu: String::new(),
        exported: String::new(),
    }
}

/// 在 `from` 里写一个会话（两段，引用一个 blob），打成包，交回包的位置、会话编号、两段的原文。
fn exported(scratch: &Scratch, from: &Side, n: u8) -> (PathBuf, SessionId, String, String) {
    let image = from.blobs.put(b"png bytes").expect("存得进");
    let id = session(n);
    let dir = from.sessions.join(id.as_str());
    fs::create_dir_all(&dir).expect("建得了");
    let first = format!("{}{}", line(1, "hi"), line(2, image.as_str()));
    let second = line(3, "bye");
    fs::write(dir.join("000000000001.jsonl"), &first).expect("写得进");
    fs::write(dir.join("000000000003.jsonl"), &second).expect("写得进");
    let out = scratch.path().join(format!("bundle-{n}.zip"));
    let parts = [Part {
        session: id.clone(),
        dir,
        upto: None,
    }];
    write(&parts, &from.blobs, &about(&id), &out).expect("打得成");
    (out, id, first, second)
}

fn sessions_in(side: &Side) -> Vec<String> {
    match fs::read_dir(&side.sessions) {
        Ok(entries) => entries
            .filter_map(Result::ok)
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .collect(),
        Err(_) => Vec::new(),
    }
}

#[test]
fn an_exported_bundle_imports_byte_for_byte_with_its_marker_and_again_is_a_no_op() {
    let scratch = Scratch::new();
    let (from, to) = (side(&scratch, "a"), side(&scratch, "b"));
    let (bundle, id, first, second) = exported(&scratch, &from, 1);
    let got = import(&bundle, &to.sessions, &to.blobs, &marker(false)).expect("导得进");
    assert_eq!(got.added, std::slice::from_ref(&id));
    let dir = to.sessions.join(id.as_str());
    assert_eq!(
        fs::read_to_string(dir.join("000000000001.jsonl")).expect("在"),
        first
    );
    assert_eq!(
        fs::read_to_string(dir.join("000000000003.jsonl")).expect("在"),
        second
    );
    let written: Marker =
        serde_json::from_slice(&fs::read(dir.join(MARKER)).expect("有标记")).expect("读得懂");
    assert_eq!(written.miyu, "0.0.0", "照清单填");
    assert!(!written.archive);
    assert!(
        to.blobs.get(&ContentHash::of(b"png bytes")).is_ok(),
        "blob 跟着进来"
    );
    let again = import(&bundle, &to.sessions, &to.blobs, &marker(false)).expect("重导不报错");
    assert!(again.added.is_empty());
    assert_eq!(again.present, [id]);
    assert_eq!(sessions_in(&to).len(), 1, "不留暂存目录");
}

#[test]
fn an_archive_stays_an_archive_when_exported_and_imported_again() {
    let scratch = Scratch::new();
    let (from, middle, last) = (
        side(&scratch, "a"),
        side(&scratch, "b"),
        side(&scratch, "c"),
    );
    let (bundle, id, _, _) = exported(&scratch, &from, 2);
    import(&bundle, &middle.sessions, &middle.blobs, &marker(true)).expect("导得进");
    let again = scratch.path().join("again.zip");
    let parts = [Part {
        session: id.clone(),
        dir: middle.sessions.join(id.as_str()),
        upto: None,
    }];
    let written = write(&parts, &middle.blobs, &about(&id), &again).expect("打得成");
    assert!(written.manifest.sessions[0].archived);
    import(&again, &last.sessions, &last.blobs, &marker(false)).expect("导得进");
    let kept: Marker = serde_json::from_slice(
        &fs::read(last.sessions.join(id.as_str()).join(MARKER)).expect("有标记"),
    )
    .expect("读得懂");
    assert!(kept.archive, "人没要存档，包说是存档，照样是存档");
}

#[test]
fn a_different_session_with_the_same_id_is_refused() {
    let scratch = Scratch::new();
    let (from, to) = (side(&scratch, "a"), side(&scratch, "b"));
    let (bundle, id, _, _) = exported(&scratch, &from, 3);
    let dir = to.sessions.join(id.as_str());
    fs::create_dir_all(&dir).expect("建得了");
    fs::write(dir.join("000000000001.jsonl"), line(1, "something else")).expect("写得进");
    let got = import(&bundle, &to.sessions, &to.blobs, &marker(false));
    assert!(
        matches!(&got, Err(ImportError::Exists(same)) if *same == id),
        "{got:?}"
    );
    assert!(dir.join(MARKER).symlink_metadata().is_err());
}

/// 照 `entries` 自己拼一个包（清单原样写）。
fn crafted(scratch: &Scratch, name: &str, manifest: &Value, entries: &[(&str, &[u8])]) -> PathBuf {
    let path = scratch.path().join(name);
    let mut zip = zip::ZipWriter::new(File::create(&path).expect("建得了"));
    let options = SimpleFileOptions::default();
    for (at, bytes) in entries {
        zip.start_file(*at, options).expect("写得进");
        zip.write_all(bytes).expect("写得进");
    }
    zip.start_file(MANIFEST, options).expect("写得进");
    zip.write_all(manifest.to_string().as_bytes())
        .expect("写得进");
    zip.finish().expect("收得了尾");
    path
}

fn one_segment_manifest(id: &SessionId, bytes: &[u8], format: u32) -> Value {
    serde_json::json!({
        "format": format, "miyu": "0.0.0", "exported": "2026-11-04T00:00:00Z", "main": id,
        "sessions": [{"id": id, "segments": [{
            "name": "000000000001.jsonl", "bytes": bytes.len(), "sha256": ContentHash::of(bytes)
        }]}],
        "blobs": [],
    })
}

#[test]
fn broken_bundles_leave_nothing_behind() {
    let scratch = Scratch::new();
    let to = side(&scratch, "b");
    let id = session(4);
    let at = format!("sessions/{id}/000000000001.jsonl");
    let good = format!("{}{}", line(1, "a"), line(2, "b"));
    let gap = format!("{}{}", line(1, "a"), line(3, "c"));
    let orphan = line(1, ContentHash::of(b"not shipped").as_str());
    let cases: Vec<(&str, Value, Vec<u8>, &str)> = vec![
        (
            "hash",
            one_segment_manifest(&id, b"other bytes\n", 1),
            good.clone().into_bytes(),
            "mismatch",
        ),
        (
            "gap",
            one_segment_manifest(&id, gap.as_bytes(), 1),
            gap.clone().into_bytes(),
            "mismatch",
        ),
        (
            "orphan",
            one_segment_manifest(&id, orphan.as_bytes(), 1),
            orphan.clone().into_bytes(),
            "mismatch",
        ),
        (
            "new",
            one_segment_manifest(&id, good.as_bytes(), FORMAT + 1),
            good.clone().into_bytes(),
            "too_new",
        ),
    ];
    for (name, manifest, bytes, expected) in cases {
        let bundle = crafted(
            &scratch,
            &format!("{name}.zip"),
            &manifest,
            &[(at.as_str(), &bytes)],
        );
        let got = import(&bundle, &to.sessions, &to.blobs, &marker(false));
        let kind = match &got {
            Err(ImportError::Mismatch(_)) => "mismatch",
            Err(ImportError::TooNew(_)) => "too_new",
            other => panic!("{name}: {other:?}"),
        };
        assert_eq!(kind, expected, "{name}");
        assert!(
            sessions_in(&to).is_empty(),
            "{name}: 什么都不留：{:?}",
            sessions_in(&to)
        );
    }
}
