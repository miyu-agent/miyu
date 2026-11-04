//! 包里人能直接看的那一份 `transcript.md`（施工 SE-1 中下中）：交了的放进包、清单记字节数和哈希；导入照清单核对它、不导进
//! 数据根，对不上的整个拒；清单没写的多出来的文件不看。

use std::io::Write;

use zip::write::SimpleFileOptions;

use super::*;

/// 一个会话一段，打成带 `transcript` 的包：交回包的位置和会话编号。
fn bundled(scratch: &Scratch, transcript: Option<&str>) -> (PathBuf, SessionId) {
    let id = session(7);
    let dir = scratch.path().join("from").join(id.as_str());
    fs::create_dir_all(&dir).expect("建得了");
    fs::write(dir.join("000000000001.jsonl"), line(1, "hi")).expect("写得进");
    let blobs = Blobs::new(scratch.path().join("from-blobs"));
    let out = scratch.path().join("with.zip");
    let mut about = about(&id);
    about.transcript = transcript.map(str::to_string);
    let parts = [Part {
        session: id.clone(),
        dir,
        upto: None,
    }];
    write(&parts, &blobs, &about, &out).expect("打得成");
    (out, id)
}

fn import_into(scratch: &Scratch, bundle: &Path, name: &str) -> Result<Imported, ImportError> {
    let root = scratch.path().join(name);
    let marker = Marker {
        archive: false,
        miyu: String::new(),
        exported: String::new(),
    };
    import(
        bundle,
        &root.join("sessions"),
        &Blobs::new(root.join("blobs")),
        &marker,
    )
}

#[test]
fn a_transcript_goes_in_listed_in_the_manifest_and_imports_untouched() {
    let scratch = Scratch::new();
    let text = "# 会话\n\n## User\n\nhi\n";
    let (out, id) = bundled(&scratch, Some(text));
    assert_eq!(entry(&out, TRANSCRIPT), text.as_bytes(), "原样放进包");
    let written: Manifest = serde_json::from_slice(&entry(&out, MANIFEST)).expect("读得懂");
    assert_eq!(
        written.transcript,
        Some(ManifestFile {
            bytes: text.len() as u64,
            sha256: ContentHash::of(text.as_bytes()),
        })
    );
    let imported = import_into(&scratch, &out, "to").expect("导得进");
    assert_eq!(imported.added, vec![id.clone()]);
    let dir = scratch.path().join("to/sessions").join(id.as_str());
    let mut files: Vec<String> = fs::read_dir(&dir)
        .expect("在")
        .filter_map(Result::ok)
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    files.sort();
    assert_eq!(files, ["000000000001.jsonl", MARKER], "不导进数据根");
    let (bare, _) = {
        let other = Scratch::new();
        let (out, id) = bundled(&other, None);
        let kept = scratch.path().join("bare.zip");
        fs::copy(&out, &kept).expect("拷得了");
        (kept, id)
    };
    assert!(
        !names(&bare).contains(&TRANSCRIPT.to_string()),
        "没交的不放"
    );
    let manifest = String::from_utf8(entry(&bare, MANIFEST)).expect("是字");
    assert!(!manifest.contains("transcript"), "清单也不写：{manifest}");
}

#[test]
fn a_transcript_that_does_not_match_fails_the_import_and_unlisted_extras_are_ignored() {
    let scratch = Scratch::new();
    let (out, id) = bundled(&scratch, Some("真的\n"));
    let manifest: Value = serde_json::from_slice(&entry(&out, MANIFEST)).expect("读得懂");
    let segment = entry(&out, &format!("sessions/{id}/000000000001.jsonl"));
    let rebuilt = |name: &str, manifest: &Value, transcript: &[u8], extra: bool| -> PathBuf {
        let path = scratch.path().join(name);
        let mut zip = zip::ZipWriter::new(File::create(&path).expect("建得了"));
        let options = SimpleFileOptions::default();
        zip.start_file(format!("sessions/{id}/000000000001.jsonl"), options)
            .expect("写得进");
        zip.write_all(&segment).expect("写得进");
        zip.start_file(TRANSCRIPT, options).expect("写得进");
        zip.write_all(transcript).expect("写得进");
        if extra {
            zip.start_file("notes.txt", options).expect("写得进");
            zip.write_all(b"extra").expect("写得进");
        }
        zip.start_file(MANIFEST, options).expect("写得进");
        zip.write_all(manifest.to_string().as_bytes())
            .expect("写得进");
        zip.finish().expect("收得了尾");
        path
    };
    let tampered = rebuilt("tampered.zip", &manifest, "假的\n".as_bytes(), false);
    match import_into(&scratch, &tampered, "a") {
        Err(ImportError::Mismatch(why)) => assert!(why.contains(TRANSCRIPT), "{why}"),
        other => panic!("{other:?}"),
    }
    assert!(
        !scratch.path().join("a/sessions").join(id.as_str()).exists(),
        "什么都不留"
    );
    let mut unlisted = manifest.clone();
    unlisted
        .as_object_mut()
        .expect("是对象")
        .remove("transcript");
    let extra = rebuilt("extra.zip", &unlisted, "随便\n".as_bytes(), true);
    let imported = import_into(&scratch, &extra, "b").expect("多出来的文件不碍事");
    assert_eq!(imported.added, vec![id]);
}
