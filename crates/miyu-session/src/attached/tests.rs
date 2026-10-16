//! 附件造成块的测试（施工 C-9）：字在前、附件照先后；内容存进这个会话的 blob，块里是哈希；图片带名字和宽高，两种都带原来
//! 的路径；没带附件的只有字，不碰磁盘；存不下的整句不发。

use miyu_kernel::id::{FileName, MediaType, SourcePath};

use super::*;

/// 一个用完就删的临时目录。
struct Scratch(std::path::PathBuf);

impl Scratch {
    fn new(name: &str) -> Scratch {
        let dir = std::env::temp_dir().join(format!("miyu-attached-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        Scratch(dir)
    }
}

impl Drop for Scratch {
    #[expect(
        clippy::let_underscore_must_use,
        reason = "删不掉就留在临时目录里，不影响测试"
    )]
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn attached(name: &str, bytes: &[u8], kind: Kind) -> Attached {
    Attached {
        name: FileName::parse(name).unwrap(),
        path: Some(SourcePath::parse(&format!("/work/{name}")).unwrap()),
        bytes: bytes.to_vec(),
        kind,
    }
}

#[tokio::test]
async fn the_words_come_first_and_each_attachment_is_stored() {
    let scratch = Scratch::new("stored");
    let blobs = Blobs::new(scratch.0.join("blobs"));
    let png = MediaType::parse("image/png").unwrap();
    let text = MediaType::parse("text/plain").unwrap();
    let picture = attached(
        "dot.png",
        b"picture bytes",
        Kind::Image {
            media_type: png.clone(),
            width: 2,
            height: 1,
        },
    );
    let note = attached(
        "note.txt",
        b"hello",
        Kind::File {
            media_type: text.clone(),
        },
    );
    let blocks = blocks(&blobs, "See these.", vec![picture, note])
        .await
        .unwrap();
    let [Block::Text(said), Block::Image(image), Block::File(file)] = blocks.as_slice() else {
        panic!("字、图、文件：{blocks:?}");
    };
    assert_eq!(said.text, "See these.");
    assert_eq!(blobs.get(&image.blob).unwrap(), b"picture bytes");
    assert_eq!(image.name.as_ref().map(FileName::as_str), Some("dot.png"));
    assert_eq!((image.width, image.height, &image.media_type), (2, 1, &png));
    assert_eq!(
        image.path.as_ref().map(SourcePath::as_str),
        Some("/work/dot.png")
    );
    assert_eq!(blobs.get(&file.blob).unwrap(), b"hello");
    assert_eq!((file.name.as_str(), &file.media_type), ("note.txt", &text));
    assert_eq!(
        file.path.as_ref().map(SourcePath::as_str),
        Some("/work/note.txt")
    );
}

#[tokio::test]
async fn without_attachments_only_the_words_and_no_disk() {
    // blob 的目录是个不存在、也建不起来的地方：碰了磁盘就会出错。
    let scratch = Scratch::new("none");
    std::fs::write(scratch.0.join("file"), b"").unwrap();
    let blobs = Blobs::new(scratch.0.join("file").join("blobs"));
    let blocks = blocks(&blobs, "Hi.", Vec::new()).await.unwrap();
    assert_eq!(
        blocks,
        [Block::Text(Text {
            text: "Hi.".to_string()
        })]
    );
    let note = attached(
        "note.txt",
        b"hello",
        Kind::File {
            media_type: MediaType::parse("text/plain").unwrap(),
        },
    );
    assert!(blocks_err(&blobs, note).await, "存不下的整句不发");
}

/// 带一个附件、存不下：交回的是出错。
async fn blocks_err(blobs: &Blobs, one: Attached) -> bool {
    blocks(blobs, "Hi.", vec![one]).await.is_err()
}

/// 临时目录下的数据根，建好了骨架（施工 3-9 六补）。
fn root_in(scratch: &Scratch) -> DataRoot {
    use miyu_store::env::{Env, Platform};
    let env = Env {
        platform: Platform::current(),
        miyu_home: Some(scratch.0.join("data").into_os_string()),
        home: None,
        xdg_cache_home: None,
        local_app_data: None,
        miyu_resources: None,
        exe: None,
    };
    let root = DataRoot::locate(&env).expect("MIYU_HOME 是绝对路径");
    root.prepare().expect("临时目录里建得了骨架");
    root
}

#[tokio::test]
async fn attachments_from_another_session_land_in_the_receiving_one() {
    let scratch = Scratch::new("kept");
    let root = root_in(&scratch);
    let owner = AccountId::parse("admin").unwrap();
    let (sender, receiver) = (
        SessionId::parse("01890000-0000-7000-8000-000000000001").unwrap(),
        SessionId::parse("01890000-0000-7000-8000-000000000002").unwrap(),
    );
    let blobs = Blobs::new(root.blobs(&owner));
    // 发话那个会话的附件目录里的一张图（落在数据根里），一份本机别处的文件，一张没有路径的图。
    let theirs = attachments::keep(
        &attachments::dir(&root, &owner, &sender),
        "截图.png",
        "image/png",
        b"png",
    )
    .unwrap();
    let blocks = blocks(
        &blobs,
        "看看",
        vec![
            Attached {
                path: Some(SourcePath::parse(&theirs.to_string_lossy()).unwrap()),
                ..attached(
                    "截图.png",
                    b"png",
                    Kind::Image {
                        media_type: MediaType::parse("image/png").unwrap(),
                        width: 1,
                        height: 1,
                    },
                )
            },
            attached(
                "notes.md",
                b"# x",
                Kind::File {
                    media_type: MediaType::parse("text/markdown").unwrap(),
                },
            ),
            Attached {
                path: None,
                ..attached(
                    "贴的.png",
                    b"png2",
                    Kind::Image {
                        media_type: MediaType::parse("image/png").unwrap(),
                        width: 1,
                        height: 1,
                    },
                )
            },
        ],
    )
    .await
    .unwrap();
    let kept = kept_for(&root, &owner, &receiver, &blobs, blocks)
        .await
        .unwrap();
    let mine = std::fs::canonicalize(attachments::dir(&root, &owner, &receiver)).unwrap();
    let path_of = |block: &Block| match block {
        Block::Image(image) => image.path.as_ref().unwrap().as_str().to_string(),
        Block::File(file) => file.path.as_ref().unwrap().as_str().to_string(),
        _ => unreachable!(),
    };
    assert_eq!(
        std::path::Path::new(&path_of(&kept[1])),
        mine.join("截图.png"),
        "数据根里的换成自己目录里的"
    );
    assert_eq!(path_of(&kept[2]), "/work/notes.md", "本机别处的照原样");
    assert_eq!(
        std::path::Path::new(&path_of(&kept[3])),
        mine.join("贴的.png")
    );
    assert_eq!(std::fs::read(mine.join("贴的.png")).unwrap(), b"png2");
}

/// 数据根里的文件换一种写法交来（这里经符号链接，Windows 上是去掉了 `\\?\` 的那种）也认得出：照真实的位置比。
#[cfg(unix)]
#[test]
fn a_path_into_the_data_root_by_another_spelling_is_still_inside() {
    let scratch = Scratch::new("spelling");
    let real = scratch.0.join("root");
    std::fs::create_dir_all(real.join("attachments")).unwrap();
    std::fs::write(real.join("attachments").join("notes.txt"), b"x").unwrap();
    let alias = scratch.0.join("alias");
    std::os::unix::fs::symlink(&real, &alias).unwrap();
    let inside = std::fs::canonicalize(&real).unwrap();
    let block = |path: &std::path::Path| {
        Block::File(File {
            blob: miyu_kernel::id::ContentHash::of(b"x"),
            name: FileName::parse("notes.txt").unwrap(),
            media_type: MediaType::parse("text/plain").unwrap(),
            path: Some(SourcePath::parse(&path.to_string_lossy()).unwrap()),
        })
    };
    assert!(foreign(
        &block(&alias.join("attachments").join("notes.txt")),
        &inside
    ));
    assert!(
        !foreign(&block(&scratch.0.join("elsewhere.txt")), &inside),
        "找不到的当本机别处的"
    );
}
