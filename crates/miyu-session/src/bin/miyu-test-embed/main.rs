//! 测试用的语义模型小程序（施工 S-7 上）：`miyu-embed` 拆进自己的仓库以后，核心的测试照它当 `miyu-embed` 拉起，不编 ONNX
//! Runtime。不随发行带出去。说的协议和 `miyu-embed` 一样（`docs/blueprint/recall.md` 第四条第 4 款）：
//!
//! - `miyu-test-embed --manifest <清单> --dir <模型文件的目录>`；载入成了先印一行 `{"ready":{"model":"local:<id>","dims":4}}`，
//!   然后标准输入一行 `{"id","text"}`、标准输出回一行 `{"id","vector"}`，读不懂的回 `{"id","error"}`；标准输入关了退出码 0。
//! - 只认手造的小模型（`tests/fixtures/embed-tiny/`，`make.py` 造的那一份）：模型文件的 SHA-256 对不上的照「载不了」说
//!   `{"error":"cannot load …"}`、退出码 1。小模型的算法照手算（[`tiny`]）：和 ONNX Runtime 算的差不过 1e-6。

mod tiny;
mod wordpiece;

use std::ffi::OsString;
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use serde_json::{Value, json};

use miyu_recall::embedding::{Manifest, Role};

/// 参数写错时印的用法。
const USAGE: &str = "usage: miyu-test-embed --manifest <file> --dir <directory>";

fn main() -> ExitCode {
    let Some((manifest, dir)) = arguments(std::env::args_os().skip(1).collect()) else {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    };
    let mut out = io::stdout().lock();
    let model = match load(&manifest, &dir) {
        Ok(model) => model,
        Err(error) => {
            if let Err(lost) = writeln!(out, "{}", json!({ "error": error })) {
                eprintln!("{error} ({lost})");
            }
            return ExitCode::from(1);
        }
    };
    let ready = json!({"ready": {"model": model.name, "dims": tiny::DIMS}});
    let served = writeln!(out, "{ready}")
        .and_then(|()| out.flush())
        .and_then(|()| serve(&model, io::stdin().lock(), &mut out));
    match served {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(1)
        }
    }
}

/// 载入好的：模型编号（`local:<id>`）、截到几个词、分词。
struct Loaded {
    name: String,
    limit: usize,
    tokenizer: wordpiece::Tokenizer,
}

/// 照清单读词表、核对模型文件是不是小模型。
fn load(manifest: &Path, dir: &Path) -> Result<Loaded, String> {
    let text = std::fs::read_to_string(manifest)
        .map_err(|error| format!("cannot read {}: {error}", manifest.display()))?;
    let manifest = Manifest::parse(&text).map_err(|error| error.to_string())?;
    let model = dir.join(&manifest.file(Role::Model).name);
    let vocab = dir.join(&manifest.file(Role::Vocab).name);
    for path in [&model, &vocab] {
        if !path.is_file() {
            return Err(format!("cannot find {}", path.display()));
        }
    }
    let bytes = std::fs::read(&model)
        .map_err(|error| format!("cannot load {}: {error}", model.display()))?;
    if !tiny::is_tiny(&bytes) || manifest.dims != tiny::DIMS {
        return Err(format!(
            "cannot load {}: not the tiny test model",
            model.display()
        ));
    }
    let words = std::fs::read_to_string(&vocab)
        .map_err(|error| format!("cannot read {}: {error}", vocab.display()))?;
    let tokenizer = wordpiece::Tokenizer::new(&words)
        .map_err(|error| format!("cannot read {}: {error}", vocab.display()))?;
    Ok(Loaded {
        name: manifest.model(),
        limit: manifest.max_tokens,
        tokenizer,
    })
}

/// 一行一行读 `input`，每行回一行。
fn serve(model: &Loaded, input: impl BufRead, mut output: impl Write) -> io::Result<()> {
    for line in input.split(b'\n') {
        let reply = answer(model, &line?);
        writeln!(output, "{reply}")?;
        output.flush()?;
    }
    Ok(())
}

/// 回一行请求。
fn answer(model: &Loaded, line: &[u8]) -> Value {
    let request: Option<Value> = std::str::from_utf8(line)
        .ok()
        .and_then(|line| serde_json::from_str(line).ok());
    let Some(request) = request else {
        return json!({"id": null, "error": "bad request"});
    };
    let Some(id) = request.get("id").and_then(Value::as_str) else {
        return json!({"id": null, "error": "bad request: no id"});
    };
    let Some(text) = request.get("text").and_then(Value::as_str) else {
        return json!({"id": id, "error": "bad request: no text"});
    };
    match tiny::embed(&model.tokenizer.encode(text, model.limit)) {
        Some(vector) => json!({"id": id, "vector": vector}),
        None => json!({"id": id, "error": "the vector is zero or not a number"}),
    }
}

/// 读 `--manifest <清单> --dir <目录>`，先后不论，各正好一次。
fn arguments(args: Vec<OsString>) -> Option<(PathBuf, PathBuf)> {
    let mut manifest = None;
    let mut dir = None;
    let mut args = args.into_iter();
    while let Some(flag) = args.next() {
        let slot = match flag.to_str() {
            Some("--manifest") => &mut manifest,
            Some("--dir") => &mut dir,
            _ => return None,
        };
        if slot.replace(PathBuf::from(args.next()?)).is_some() {
            return None;
        }
    }
    Some((manifest?, dir?))
}
