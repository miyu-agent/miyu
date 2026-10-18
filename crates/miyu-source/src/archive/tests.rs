//! 包文件（施工 G-1）：打出来可重现；解开和原来一样；链接、跑出去的路径、不止一个最上层、太大的都拒。

use super::*;
use crate::test_support::{Scratch, put};

/// 一个小包：清单、一个 `bin/` 下的程序、一个空文件夹、一个 `.git`。
fn sample(root: &Path) -> PathBuf {
    let folder = root.join("pudding");
    put(
        &folder,
        "package.toml",
        b"[package]\nname = { en = \"Pudding\" }\n",
    );
    put(&folder, "bin/pudding", b"#!/bin/sh\necho hi\n");
    put(&folder, "skills/sweet/SKILL.md", b"---\nname: sweet\n---\n");
    put(&folder, ".git/HEAD", b"ref: refs/heads/main\n");
    fs::create_dir_all(folder.join("empty")).expect("建得了");
    folder
}

/// 一项：名字、种类、内容。
type Raw<'a> = (&'a str, tar::EntryType, &'a [u8]);

/// 用 tar 自己写一个包文件：每一项照原样写名字（不经过 `tar` 的路径检查），好造坏的。
fn raw(to: &Path, items: &[Raw<'_>]) {
    let encoder = zstd::Encoder::new(File::create(to).expect("建得了"), 3).expect("压得了");
    let mut builder = tar::Builder::new(encoder);
    for (name, kind, bytes) in items {
        let mut header = tar::Header::new_gnu();
        header.as_old_mut().name[..name.len()].copy_from_slice(name.as_bytes());
        header.set_entry_type(*kind);
        header.set_size(bytes.len() as u64);
        header.set_mode(0o644);
        if *kind == tar::EntryType::Symlink {
            header.set_link_name("/etc/passwd").expect("写得了");
        }
        header.set_cksum();
        builder.append(&header, *bytes).expect("写得进");
    }
    builder.into_inner().expect("写完").finish().expect("压完");
}

#[test]
fn the_same_folder_packs_to_the_same_bytes() {
    let scratch = Scratch::new();
    let one = sample(&scratch.path().join("one"));
    let first = pack(&one, "pudding", &scratch.path().join("a.miyu")).expect("打得成");
    // 换一个地方、换一个时刻再写一份同样的内容。
    let two = sample(&scratch.path().join("two"));
    std::thread::sleep(std::time::Duration::from_millis(20));
    put(
        &two,
        "package.toml",
        b"[package]\nname = { en = \"Pudding\" }\n",
    );
    let second = pack(&two, "pudding", &scratch.path().join("b.miyu")).expect("打得成");
    assert_eq!(first.sha256, second.sha256, "只看内容");
    assert_eq!(
        fs::read(&first.path).expect("在"),
        fs::read(&second.path).expect("在")
    );
    assert_eq!(first.size, fs::metadata(&first.path).expect("在").len());
    assert_eq!(first.sha256.len(), 64);
    assert!(
        !scratch.path().join("a.miyu.partial").exists(),
        "写了一半的那一份换过去了"
    );
    put(
        &two,
        "package.toml",
        b"[package]\nname = { en = \"Pudding 2\" }\n",
    );
    let third = pack(&two, "pudding", &scratch.path().join("c.miyu")).expect("打得成");
    assert_ne!(first.sha256, third.sha256, "内容变了哈希就变");
}

#[test]
fn unpacking_gives_back_the_folder_without_git() {
    let scratch = Scratch::new();
    let folder = sample(scratch.path());
    let packed = pack(&folder, "pudding", &scratch.path().join("p.miyu")).expect("打得成");
    let into = scratch.path().join("out");
    let unpacked = unpack(&packed.path, &into).expect("解得开");
    assert_eq!(unpacked.id, "pudding");
    assert_eq!(unpacked.folder, into.join("pudding"));
    for file in ["package.toml", "bin/pudding", "skills/sweet/SKILL.md"] {
        assert_eq!(
            fs::read(unpacked.folder.join(file)).expect("在"),
            fs::read(folder.join(file)).expect("在"),
            "{file}"
        );
    }
    assert!(unpacked.folder.join("empty").is_dir(), "空文件夹也在");
    assert!(!unpacked.folder.join(".git").exists(), ".git 不打进去");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = |file: &str| {
            fs::metadata(unpacked.folder.join(file))
                .expect("在")
                .permissions()
                .mode()
                & 0o777
        };
        assert_eq!(mode("bin/pudding"), 0o755, "bin/ 下的能执行");
        assert_eq!(mode("package.toml"), 0o644);
    }
    let busy = scratch.path().join("busy");
    put(&busy, "other.txt", b"x");
    let again = unpack(&packed.path, &busy);
    assert!(
        matches!(again, Err(Error::Io(ref error)) if error.kind() == io::ErrorKind::AlreadyExists),
        "解进不是空的文件夹的拒：{again:?}"
    );
}

#[cfg(unix)]
#[test]
fn an_executable_bit_outside_bin_is_kept_and_links_are_refused() {
    use std::os::unix::fs::PermissionsExt;
    let scratch = Scratch::new();
    let folder = sample(scratch.path());
    put(&folder, "tools/run.sh", b"#!/bin/sh\n");
    fs::set_permissions(
        folder.join("tools/run.sh"),
        fs::Permissions::from_mode(0o700),
    )
    .expect("改得了");
    let packed = pack(&folder, "pudding", &scratch.path().join("p.miyu")).expect("打得成");
    let unpacked = unpack(&packed.path, &scratch.path().join("out")).expect("解得开");
    let mode = fs::metadata(unpacked.folder.join("tools/run.sh"))
        .expect("在")
        .permissions()
        .mode();
    assert_eq!(mode & 0o777, 0o755);

    std::os::unix::fs::symlink("/etc/passwd", folder.join("link")).expect("建得了链接");
    let refused = pack(&folder, "pudding", &scratch.path().join("q.miyu"));
    assert!(
        matches!(refused, Err(Error::NotPlain(ref path)) if path == "link"),
        "{refused:?}"
    );
    assert!(!scratch.path().join("q.miyu").exists());
}

#[test]
fn file_names_carry_the_version_and_the_target() {
    assert_eq!(file_name("embed", "1.0.0", None), "embed-1.0.0.miyu");
    assert_eq!(
        file_name("onebot", "0.2.0", Some(Target::MacosAarch64)),
        "onebot-0.2.0-macos-aarch64.miyu"
    );
}

#[test]
fn bad_packages_are_refused() {
    let scratch = Scratch::new();
    let cases: [(&str, &[Raw<'_>]); 5] = [
        (
            "两个最上层",
            &[
                ("a/package.toml", tar::EntryType::Regular, b"x"),
                ("b/package.toml", tar::EntryType::Regular, b"x"),
            ],
        ),
        (
            "跑到外面",
            &[
                ("a/package.toml", tar::EntryType::Regular, b"x"),
                ("a/../../evil", tar::EntryType::Regular, b"x"),
            ],
        ),
        ("绝对路径", &[("/tmp/evil", tar::EntryType::Regular, b"x")]),
        (
            "链接",
            &[
                ("a/package.toml", tar::EntryType::Regular, b"x"),
                ("a/link", tar::EntryType::Symlink, b""),
            ],
        ),
        ("空的", &[]),
    ];
    for (n, (why, items)) in cases.iter().enumerate() {
        let file = scratch.path().join(format!("{n}.miyu"));
        raw(&file, items);
        let into = scratch.path().join(format!("out-{n}"));
        let got = unpack(&file, &into);
        assert!(
            matches!(got, Err(Error::BadPath(_) | Error::NotPlain(_))),
            "{why}：{got:?}"
        );
        assert!(!scratch.path().join("evil").exists(), "{why}：没写到外面");
    }
}

#[test]
fn too_many_entries_or_bytes_are_refused() {
    let scratch = Scratch::new();
    let big = vec![0_u8; 1024];
    let file = scratch.path().join("big.miyu");
    raw(
        &file,
        &[
            ("a/one", tar::EntryType::Regular, &big),
            ("a/two", tar::EntryType::Regular, &big),
        ],
    );
    let bytes = unpack_within(&file, &scratch.path().join("x"), 10, 1500);
    assert!(matches!(bytes, Err(Error::TooBig)), "{bytes:?}");
    let entries = unpack_within(&file, &scratch.path().join("y"), 1, u64::MAX);
    assert!(matches!(entries, Err(Error::TooBig)), "{entries:?}");
    let fine = unpack_within(&file, &scratch.path().join("z"), 2, 2048);
    assert!(fine.is_ok(), "{fine:?}");
}

#[test]
fn not_a_package_file_is_an_error() {
    let scratch = Scratch::new();
    let file = scratch.path().join("text.miyu");
    fs::write(&file, b"just some text").expect("写得进");
    assert!(matches!(
        unpack(&file, &scratch.path().join("out")),
        Err(Error::Io(_))
    ));
}

#[test]
fn entries_are_sorted_with_zero_times_and_owners() {
    let scratch = Scratch::new();
    let folder = sample(scratch.path());
    put(&folder, "a-b.txt", b"x");
    put(&folder, "Z.txt", b"x");
    let packed = pack(&folder, "pudding", &scratch.path().join("p.miyu")).expect("打得成");
    let decoder = zstd::Decoder::new(File::open(&packed.path).expect("在")).expect("解得了");
    let mut archive = tar::Archive::new(decoder);
    let mut names = Vec::new();
    for entry in archive.entries().expect("读得了") {
        let entry = entry.expect("读得了");
        let header = entry.header();
        assert_eq!(header.mtime().expect("有"), 0);
        assert_eq!(header.uid().expect("有"), 0);
        assert_eq!(header.gid().expect("有"), 0);
        let name = entry.path().expect("有").display().to_string();
        names.push(name.trim_end_matches('/').to_string());
    }
    assert_eq!(
        names,
        [
            "pudding",
            "pudding/Z.txt",
            "pudding/a-b.txt",
            "pudding/bin",
            "pudding/bin/pudding",
            "pudding/empty",
            "pudding/package.toml",
            "pudding/skills",
            "pudding/skills/sweet",
            "pudding/skills/sweet/SKILL.md",
        ],
        "照路径的字节排，文件夹在里面的东西前面"
    );
}
