//! 抽字（施工 K-5，`docs/blueprint/knowledge.md`「怎么走」第二条第 5、6 款）：中英文的 PDF 一页一份、头尾空白去掉、部首的码位
//! 换回平常的字；只有画的一页是空的；不是 PDF 的说为什么。网页照标题写 `#`、脚本样式不要。小程序照参数抽、照 JSON 交回，参数
//! 不对、读不了的退出码 1、说为什么。

use std::path::PathBuf;
use std::process::Command;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

#[test]
fn a_pdf_is_extracted_page_by_page_with_plain_characters() {
    let bytes = std::fs::read(fixture("cat.pdf")).expect("读得了");
    assert_eq!(
        miyu_extract::pdf(&bytes).expect("抽得出"),
        [
            "团子是一只橘猫，每天吃鸡胸肉。",
            "Vaccines are given every October.",
            "第三页：体重 4.2 公斤。",
        ]
    );
    let blank = std::fs::read(fixture("blank.pdf")).expect("读得了");
    assert_eq!(
        miyu_extract::pdf(&blank).expect("抽得出"),
        [""],
        "只有画的一页是空的"
    );
    let broken = miyu_extract::pdf("这不是 PDF".as_bytes()).expect_err("抽不出");
    assert!(broken.starts_with("unreadable PDF"), "{broken}");
}

#[test]
fn a_web_page_becomes_text_with_headings() {
    let html = "<html><head><title>标题</title><style>.x{color:red}</style><script>var hidden = 1;</script></head>\
        <body><h1>猫</h1><p>团子是<b>橘猫</b>，<a href=\"x.html\">链接</a>。</p><h2>疫苗</h2><p>每年十月打针。</p></body></html>";
    assert_eq!(
        miyu_extract::html(html.as_bytes()).expect("换得了"),
        "# 猫\n\n团子是橘猫，[链接]。\n\n## 疫苗\n\n每年十月打针。\n"
    );
}

#[test]
fn the_program_answers_in_json_or_says_why() {
    let program = env!("CARGO_BIN_EXE_miyu-extract");
    let done = Command::new(program)
        .arg("pdf")
        .arg(fixture("cat.pdf"))
        .output()
        .expect("跑得起来");
    assert!(done.status.success());
    let pages: Vec<String> = serde_json::from_slice(&done.stdout).expect("是 JSON");
    assert_eq!(pages.len(), 3);
    let cat = fixture("cat.pdf").display().to_string();
    for args in [
        vec!["pdf"],
        vec!["pdf", cat.as_str(), "多一个"],
        vec!["doc", "x"],
        vec!["pdf", "/没有这个文件.pdf"],
    ] {
        let done = Command::new(program)
            .args(&args)
            .output()
            .expect("跑得起来");
        assert_eq!(done.status.code(), Some(1), "{args:?}");
        assert!(done.stdout.is_empty());
        assert!(!done.stderr.is_empty(), "说为什么：{args:?}");
    }
}
