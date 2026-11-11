//! 造对话（施工 V-42 的对照测试）：随机数、一段对话的状态、各种消息、驱动、执行器那样取 blob。

use std::collections::{BTreeMap, BTreeSet};

use miyu_drivers::openai_chat::{
    Compat, Continuation, ContinuationField, OutputLimit, ReasoningField, ReasoningReplay,
};
use miyu_drivers::{
    Anthropic, BlobBytes, Call, Driver, DriverTextSources, DriverTexts, Inputs, Memo, OpenAiChat,
    OpenAiResponses,
};
use miyu_kernel::block::{Block, File, Image, Reasoning, Text, ToolCall};
use miyu_kernel::id::{CallId, ContentHash, FileName, MediaType, ModelName, SourcePath};
use miyu_kernel::request::{Message, Request, ToolSpec};

use crate::support::{private, raw, texts};

/// 小而快的随机数（xorshift64*）：照种子定，换平台也一样。
pub(super) struct Rng(u64);

impl Rng {
    pub(super) fn new(seed: u64) -> Rng {
        Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
    }

    pub(super) fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    pub(super) fn below(&mut self, n: usize) -> usize {
        usize::try_from(self.next() % (n.max(1) as u64)).unwrap_or(0)
    }

    pub(super) fn chance(&mut self, percent: u64) -> bool {
        self.next() % 100 < percent
    }

    pub(super) fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.below(items.len())]
    }
}

/// 一段对话的状态：像内核那样一次次交出请求。
pub(super) struct Talk {
    pub(super) rng: Rng,
    pub(super) request: Request,
    /// 存着的 blob：哈希 → 字节。
    pub(super) store: BTreeMap<ContentHash, Vec<u8>>,
    /// 用到过的 blob（删掉的还在这里，好放回去）。
    pub(super) known: BTreeMap<ContentHash, Vec<u8>>,
    /// 这一次发给谁。
    pub(super) driver: usize,
    pub(super) call: Call,
    /// 占位的几句：出厂的，或者另一套（retext）。
    pub(super) retexted: bool,
    /// 下一个调用编号的序号。
    pub(super) seq: u64,
    /// 最近几个调用编号：偶尔拿来重用。
    pub(super) recent: Vec<CallId>,
    /// 会不会重用调用编号。
    pub(super) repeats: bool,
}

/// 驱动：openai-chat 的几套开关、anthropic、openai-responses。
pub(super) fn driver(which: usize, texts: DriverTexts) -> Box<dyn Driver> {
    let replay = |reasoning| Compat {
        reasoning,
        ..Compat::default()
    };
    match which {
        0 => Box::new(OpenAiChat::new(Compat::deepseek(), texts)),
        1 => Box::new(OpenAiChat::new(Compat::default(), texts)),
        2 => Box::new(OpenAiChat::new(replay(ReasoningReplay::Drop), texts)),
        3 => Box::new(OpenAiChat::new(
            Compat {
                reasoning: ReasoningReplay::Replay {
                    field: ReasoningField::Reasoning,
                    always: false,
                },
                output_limit: OutputLimit::MaxCompletionTokens,
                stream_usage: false,
                continuation: Continuation::Prefix {
                    field: ContinuationField::Partial,
                    path: "/v1/partial".to_string(),
                },
                toggle: None,
            },
            texts,
        )),
        4 => Box::new(Anthropic::new(texts)),
        _ => Box::new(OpenAiResponses::new(texts)),
    }
}

/// 驱动有几种。
pub(super) const DRIVERS: usize = 6;

/// 另一套占位（retext）：没有文本文件、带名字的图、转述、路径那几句。
pub(super) fn other_texts() -> DriverTexts {
    DriverTexts::new(DriverTextSources {
        image_omitted: "[an image was here]",
        file_omitted: include_str!("../../../../resources/core/drivers/file-omitted.txt"),
        no_output: "(nothing came back)",
        tool_attachments: "Attached below:",
        tool_attachments_only: "(see the attachments)",
        text_file: None,
        image_name: None,
        image_description: None,
        attached_path: None,
        open_path: None,
        path_only: None,
    })
    .expect("这一套占位用得了")
}

/// 会转义的、多字节的字混在里面。
pub(super) const WORDS: [&str; 14] = [
    "hello",
    "你好",
    "\"quoted\"",
    "back\\slash",
    "tab\there",
    "line\nbreak",
    "\u{1}ctl",
    "</file>",
    "😀",
    "ends with newline\n",
    "{\"json\":true}",
    "",
    "αβγ",
    "\u{7f}del",
];

impl Talk {
    pub(super) fn new(seed: u64) -> Talk {
        let mut rng = Rng::new(seed);
        let driver = rng.below(DRIVERS);
        let mut talk = Talk {
            rng,
            request: Request {
                tools: Vec::new(),
                system: String::new(),
                messages: Vec::new(),
                stable: 0,
                continuation: false,
                described: BTreeMap::new(),
                output_cap: None,
                used: None,
                tail: None,
            },
            store: BTreeMap::new(),
            known: BTreeMap::new(),
            driver,
            call: Call {
                model: ModelName::parse("m-1").expect("模型名合写法"),
                max_output: Some(4096),
                inputs: Inputs {
                    images: true,
                    pdf: true,
                },
                effort: None,
                temperature: None,
            },
            retexted: false,
            seq: 1,
            recent: Vec::new(),
            repeats: true,
        };
        talk.request.system = talk.words(3);
        talk.request.tools = vec![talk.tool("read"), talk.tool("shell")];
        talk
    }

    pub(super) fn words(&mut self, n: usize) -> String {
        let count = 1 + self.rng.below(n);
        (0..count)
            .map(|_| *self.rng.pick(&WORDS))
            .collect::<Vec<_>>()
            .join(" ")
    }

    pub(super) fn tool(&mut self, name: &str) -> ToolSpec {
        ToolSpec {
            name: name.to_string(),
            description: self.words(4),
            parameters: raw(r#"{"type":"object","properties":{"path":{"type":"string"}}}"#),
        }
    }

    pub(super) fn text(&mut self) -> Block {
        Block::Text(Text {
            text: self.words(5),
        })
    }

    /// 一个 blob：存起来，交回哈希。
    pub(super) fn blob(&mut self, bytes: Vec<u8>) -> ContentHash {
        let hash = ContentHash::of(&bytes);
        self.store.insert(hash.clone(), bytes.clone());
        self.known.insert(hash.clone(), bytes);
        hash
    }

    pub(super) fn path(&mut self) -> Option<SourcePath> {
        self.rng
            .chance(30)
            .then(|| SourcePath::parse("/home/someone/a \"pic\".png").expect("路径合写法"))
    }

    pub(super) fn image(&mut self) -> Block {
        let content = format!("img-{}", self.rng.next() % 6).into_bytes();
        let media = *self
            .rng
            .pick(&["image/png", "image/jpeg", "image/bmp", "image/webp"]);
        let name = self
            .rng
            .chance(50)
            .then(|| FileName::parse("shot \"1\".png").expect("文件名合写法"));
        Block::Image(Image {
            blob: self.blob(content),
            name,
            media_type: MediaType::parse(media).expect("媒体类型合写法"),
            width: 640,
            height: 480,
            path: self.path(),
        })
    }

    pub(super) fn file(&mut self) -> Block {
        let (content, media, name): (Vec<u8>, &str, &str) = match self.rng.below(3) {
            0 => (
                format!("%PDF-{}", self.rng.next() % 5).into_bytes(),
                "application/pdf",
                "doc.pdf",
            ),
            1 => (
                format!("notes {}\n\"x\"\\ </file> 😀", self.words(3)).into_bytes(),
                "text/plain",
                "notes.txt",
            ),
            _ => (
                vec![0, 159, 146, 150, 0, 1],
                "application/octet-stream",
                "a.bin",
            ),
        };
        Block::File(File {
            blob: self.blob(content),
            name: FileName::parse(name).expect("文件名合写法"),
            media_type: MediaType::parse(media).expect("媒体类型合写法"),
            path: self.path(),
        })
    }

    pub(super) fn blocks(&mut self, media: bool) -> Vec<Block> {
        let count = self.rng.below(4);
        (0..count)
            .map(|_| match self.rng.below(if media { 6 } else { 2 }) {
                0 | 1 => self.text(),
                2 | 3 => self.image(),
                _ => self.file(),
            })
            .collect()
    }

    /// 一个新的调用编号；偶尔重用最近的一个（编号重了：后来的那个说了算）。
    pub(super) fn call_id(&mut self) -> CallId {
        if self.repeats && !self.recent.is_empty() && self.rng.chance(4) {
            return *self.rng.pick(&self.recent);
        }
        let id = CallId::parse(&format!("call_{}_1", self.seq)).expect("调用编号合写法");
        self.seq += 1;
        self.recent.push(id);
        id
    }

    /// 一条 assistant：正文、各家的思考、工具调用（各家的编号）。交回调用编号。
    pub(super) fn assistant(&mut self) -> Vec<CallId> {
        let mut blocks = Vec::new();
        let mut calls = Vec::new();
        for _ in 0..1 + self.rng.below(4) {
            match self.rng.below(5) {
                0 | 1 => blocks.push(self.text()),
                2 => {
                    let (family, data) = match self.rng.below(6) {
                        0 => ("openai-chat", r#"{"field":"reasoning_content"}"#),
                        1 => ("openai-chat", r#"{"field":"reasoning"}"#),
                        2 => ("anthropic", r#"{"signature":"sig\"1"}"#),
                        3 => ("anthropic", r#"{"redacted":"opaque"}"#),
                        4 => (
                            "openai-responses",
                            r#"{"id":"rs_1","encrypted_content":"enc"}"#,
                        ),
                        _ => ("openai-chat", r#"{"other":1}"#),
                    };
                    let text = self.words(3);
                    blocks.push(Block::Reasoning(Reasoning {
                        text,
                        private: self.rng.chance(80).then(|| private(family, data)),
                    }));
                }
                _ => {
                    let call_id = self.call_id();
                    let provider = self.rng.next() % 1000;
                    let data = match self.rng.below(4) {
                        0 => Some(("openai-chat", format!(r#"{{"id":"call_{provider}"}}"#))),
                        1 => Some(("anthropic", format!(r#"{{"id":"toolu_{provider}"}}"#))),
                        2 => Some((
                            "openai-responses",
                            format!(r#"{{"call_id":"fc_{provider}"}}"#),
                        )),
                        _ => None,
                    };
                    let args =
                        *self
                            .rng
                            .pick(&[r#"{"path":"src"}"#, "", r#"{"path":"\"q\""}"#, "[1]"]);
                    blocks.push(Block::ToolCall(ToolCall {
                        call_id,
                        name: "read".to_string(),
                        args: args.to_string(),
                        private: data.map(|(family, data)| private(family, &data)),
                    }));
                    calls.push(call_id);
                }
            }
        }
        self.request.messages.push(Message::Assistant { blocks });
        calls
    }

    pub(super) fn tool_result(&mut self, call_id: CallId) {
        let blocks = self.blocks(true);
        let error = self.rng.chance(15);
        self.request.messages.push(Message::Tool {
            call_id,
            error,
            blocks,
        });
    }

    pub(super) fn user(&mut self) {
        let mut blocks = self.blocks(true);
        if blocks.is_empty() {
            blocks.push(self.text());
        }
        self.request.messages.push(Message::User { blocks });
    }

    /// 这一次的占位。
    pub(super) fn texts(&self) -> DriverTexts {
        match self.retexted {
            true => other_texts(),
            false => texts(),
        }
    }

    /// 编一次：两边比，交回有没有出错。
    pub(super) fn compare(&self, memo: &mut Memo, seed: u64, step: usize, what: &str) -> bool {
        let driver = driver(self.driver, self.texts());
        let got = fetched(driver.as_ref(), &self.request, &self.call, &self.store);
        let whole = driver.fragments(&self.request, &self.call, &got);
        let kept = driver.fragments_memo(&self.request, &self.call, &got, memo);
        assert!(
            whole == kept,
            "种子 {seed} 第 {step} 次（{what}，驱动 {}）：留着编的和整份编的不一样",
            self.driver
        );
        whole.is_err()
    }
}

/// 照执行器的取法（`route/exchange.rs` 的 `fetch`）：要整份的给字节，别的只给大小；存里没有的不给。
pub(super) struct Fetched(BTreeMap<ContentHash, (u64, Option<Vec<u8>>)>);

impl BlobBytes for Fetched {
    fn bytes(&self, hash: &ContentHash) -> Option<&[u8]> {
        self.0.get(hash)?.1.as_deref()
    }

    fn size(&self, hash: &ContentHash) -> Option<u64> {
        self.0.get(hash).map(|(size, _)| *size)
    }
}

pub(super) fn fetched(
    driver: &dyn Driver,
    request: &Request,
    call: &Call,
    store: &BTreeMap<ContentHash, Vec<u8>>,
) -> Fetched {
    let whole: BTreeSet<ContentHash> = driver.blobs_whole(request, call);
    Fetched(
        driver
            .blobs_needed(request, call)
            .into_iter()
            .filter_map(|hash| {
                let bytes = store.get(&hash)?;
                let size = bytes.len() as u64;
                let kept = whole.contains(&hash).then(|| bytes.clone());
                Some((hash, (size, kept)))
            })
            .collect(),
    )
}
