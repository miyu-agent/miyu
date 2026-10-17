//! 清单的读法（`recall.md` 第四条第 2 款）：合写法的读得出；少格、多格、`role` 重了或缺了、不认识的取法、数不对、文件名带
//! 目录、不是 TOML 的都拒，说清楚哪里不对。出厂那一份读不读得出在 `miyu-embed` 的测试里（要读文件）。

use super::*;

/// 一份合写法的清单，`top` 接在最上面那几格后面，`files` 是 `[[files]]` 那几段。
fn manifest(top: &str, files: &str) -> String {
    format!("id = \"m\"\ndims = 4\npooling = \"cls\"\nmax_tokens = 8\n{top}\n{files}")
}

const BOTH: &str = "[[files]]\nrole = \"model\"\nname = \"model.onnx\"\nsha256 = \"00\"\nsize = 1\n\n[[files]]\nrole = \"vocab\"\nname = \"vocab.txt\"\nsha256 = \"11\"\nsize = 2\n";

#[test]
fn a_good_one_parses() {
    let manifest = Manifest::parse(&manifest("", BOTH)).expect("合写法");
    assert_eq!(manifest.file(Role::Vocab).sha256, "11");
    assert_eq!(manifest.files.len(), 2);
}

#[test]
fn what_is_wrong_is_said() {
    let wrong = [
        (
            manifest("", "").replace("dims = 4\n", ""),
            "missing field `dims`",
        ),
        (manifest("color = \"red\"", BOTH), "unknown field `color`"),
        (
            manifest("", BOTH).replace("pooling = \"cls\"", "pooling = \"mean\""),
            "unknown variant `mean`",
        ),
        (
            manifest("", BOTH).replace("role = \"vocab\"", "role = \"model\""),
            "two files are model",
        ),
        (
            manifest("", &BOTH[..BOTH.find("\n\n").expect("两段")]),
            "no file is vocab",
        ),
        (
            manifest("", BOTH).replace("dims = 4", "dims = 0"),
            "dims must be at least 1",
        ),
        (
            manifest("", BOTH).replace("max_tokens = 8", "max_tokens = 1"),
            "max_tokens must be at least 2",
        ),
        (
            manifest("", BOTH).replace("name = \"vocab.txt\"", "name = \"../vocab.txt\""),
            "file name ../vocab.txt is not a plain name",
        ),
        (
            manifest("", BOTH).replace("id = \"m\"", "id = \"../m\""),
            "id ../m is not a plain name",
        ),
        ("not toml [".to_string(), "invalid"),
    ];
    for (text, said) in wrong {
        let error = Manifest::parse(&text).expect_err(said).to_string();
        assert!(
            error.contains(said),
            "说的是「{error}」，要有「{said}」：\n{text}"
        );
    }
}

const MERGES: &str =
    "\n[[files]]\nrole = \"merges\"\nname = \"merges.txt\"\nsha256 = \"22\"\nsize = 3\n";

const BPE: &str = "[tokenizer]\nkind = \"bpe\"\nstart = \"<s>\"\nend = \"</s>\"\n";

/// 分词（R-5 六补）：不写的照 WordPiece；BPE 的带前后两个词、要正好一个 `merges`；WordPiece 的不能有。
#[test]
fn the_tokenizer_decides_the_files() {
    let old = Manifest::parse(&manifest("", BOTH)).expect("旧的照读");
    assert_eq!(old.tokenizer, Tokenizer::Wordpiece);
    let bpe = Manifest::parse(&manifest(BPE, &format!("{BOTH}{MERGES}"))).expect("合写法");
    assert_eq!(
        bpe.tokenizer,
        Tokenizer::Bpe {
            start: "<s>".to_string(),
            end: "</s>".to_string()
        }
    );
    assert_eq!(bpe.file(Role::Merges).name, "merges.txt");
    let wrong = [
        (manifest(BPE, BOTH), "no file is merges"),
        (
            manifest(BPE, &format!("{BOTH}{MERGES}{MERGES}")),
            "two files are merges",
        ),
        (
            manifest("", &format!("{BOTH}{MERGES}")),
            "a wordpiece tokenizer has no merges file",
        ),
        (
            manifest(
                "[tokenizer]\nkind = \"wordpiece\"\n",
                &format!("{BOTH}{MERGES}"),
            ),
            "a wordpiece tokenizer has no merges file",
        ),
        (
            manifest(&BPE.replace("bpe", "unigram"), BOTH),
            "unknown variant `unigram`",
        ),
        (
            manifest(&BPE.replace("end = \"</s>\"\n", ""), BOTH),
            "missing field `end`",
        ),
        (
            manifest(&format!("{BPE}lowercase = true\n"), BOTH),
            "unknown field `lowercase`",
        ),
    ];
    for (text, said) in wrong {
        let error = Manifest::parse(&text).expect_err(said).to_string();
        assert!(
            error.contains(said),
            "说的是「{error}」，要有「{said}」：\n{text}"
        );
    }
}
