use miyu_drivers::Inputs;
use miyu_http::testkit::Server;
use miyu_kernel::id::ModelName;

use super::golden::{cases, ready};
use super::samples::media;
use super::*;

fn call(max_output: Option<u32>) -> Call {
    Call {
        model: ModelName::parse("m").expect("合模型名的写法"),
        max_output,
        inputs: Inputs::default(),
        effort: None,
        temperature: None,
    }
}

/// 快满了的主请求（施工 6-11 三补）：输出上限压到请求给的；原来更小的、请求没给的照原样；原来没写的照它写。
#[test]
fn the_output_is_capped_only_when_the_request_says_so() {
    assert_eq!(capped(&call(Some(8192)), None), None, "平常的照原样");
    assert_eq!(
        capped(&call(Some(8192)), Some(9000)),
        None,
        "原来更小的照原样"
    );
    assert_eq!(capped(&call(Some(8192)), Some(8192)), None);
    assert_eq!(
        capped(&call(Some(8192)), Some(5000)).and_then(|call| call.max_output),
        Some(5000)
    );
    assert_eq!(
        capped(&call(None), Some(5000)).and_then(|call| call.max_output),
        Some(5000),
        "原来没写的照它写：供应商默认的输出可能超窗口"
    );
}

/// 编码要的 blob 取不出来（施工 V-38 以后照旧）：没有的、坏了的，照原样发的图和照字放进消息的文件都报「其他」，说是哪一个，
/// 一个字节都不发，也不报「发出去了」。
#[tokio::test]
async fn a_blob_that_cannot_be_read_is_reported_and_nothing_is_sent() {
    let dir = std::env::temp_dir().join(format!("miyu-v38-missing-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("建得了临时目录");
    std::fs::write(
        dir.join(miyu_store::root::MARKER),
        miyu_store::root::MARKER_TEXT,
    )
    .expect("写得进标记");
    let media = media();
    let png = ContentHash::of(&media.png);
    let notes = ContentHash::of(&media.notes);
    // 丢了的：图没存，文本文件存了；坏了的：图存了又被改过（另造的 `Blobs` 没核对过），文本文件没存。
    for (gone, corrupt) in [(&png, false), (&notes, false), (&png, true)] {
        let store = dir.join(format!("blobs-{}-{corrupt}", gone.hex()));
        let blobs = Blobs::new(store.clone());
        for bytes in [&media.png, &media.jpeg, &media.pdf, &media.notes] {
            if ContentHash::of(bytes) != *gone || corrupt {
                blobs.put(bytes).expect("存得进 blob");
            }
        }
        if corrupt {
            let mut tampered = media.png.clone();
            tampered[100] ^= 1;
            std::fs::write(blobs.path(gone), tampered).expect("改得了");
        }
        let blobs = Blobs::new(store);
        // openai-chat 默认那一套看不了图：图那一回不用 blob。
        for (driver, call, request, placeholders) in cases(&media) {
            let sees = call.inputs.images;
            let server = Server::start(Vec::new()).await;
            let ready = ready(&server, driver, call, placeholders);
            let mut sent = false;
            let exchanged = exchange(
                &ready,
                Cow::Borrowed(&request),
                &blobs,
                std::future::pending(),
                |_| sent = true,
            )
            .await;
            if *gone == png && !sees {
                continue;
            }
            let Exchanged::Missing(error) = exchanged else {
                panic!("{gone} 取不出来却发了");
            };
            assert_eq!(error.class, ErrorClass::Unclassified);
            assert!(error.message.contains(gone.as_str()), "{}", error.message);
            assert!(!sent, "没报发出去了");
            assert!(server.received().is_empty(), "一个字节都没发");
        }
    }
    miyu_store::testkit::remove_tree(&dir);
}
