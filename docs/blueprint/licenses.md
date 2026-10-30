## 许可证

### 是什么

仓库用 AGPL-3.0-or-later（2026-10-26 项目主人定，原来是 GPL-3.0-or-later：Miyu 以后主要是在服务器上给人连着用，GPL 管不到「改了只在网上提供服务」）。给别人照着写东西用的几处分出来用 Apache-2.0（分层，下面「对外的样子」）。第三方依赖的许可证都要能和 AGPL 合在一起发，门禁的「许可证」一项查：能和 GPL-3.0 合的宽松许可证也都能和 AGPL-3.0 合（AGPL 第 13 条），所以能用的那张表不变。

### 在哪

| 文件 | 管什么 |
|---|---|
| `LICENSE` | AGPL-3.0 全文，照自由软件基金会发的原样（`https://www.gnu.org/licenses/agpl-3.0.txt`）：34523 字节，sha256 以 `0d96a4ff` 开头 |
| `LICENSES/Apache-2.0.txt` | Apache-2.0 全文，照 Apache 软件基金会发的原样（`https://www.apache.org/licenses/LICENSE-2.0.txt`）：11358 字节，sha256 以 `cfc7749b` 开头；管 README 列的那几个目录 |
| `Cargo.toml` | 工作区的 `license = "AGPL-3.0-or-later"`，每个 crate 照它（`license.workspace = true`） |
| `README.md` 的「许可证」一节 | 对外怎么说 |
| `xtask/src/licenses.rs` | 门禁的「许可证」一项 |
| `THIRD-PARTY-LICENSES.txt` | 第三方许可证清单：编进程序的第三方包和资源里第三方数据的版权、许可证原文，随安装包发（施工 AU-2） |
| `xtask/src/notices.rs`、`xtask/src/notices/` | 生成这份清单（`cargo xtask notices`），门禁查它是不是最新的；`notices/` 下是包没带许可证文件时用的三份标准原文 |

### 对外的样子

README 那一节（例子）：

```text
## 许可证

核心（这个仓库自己写的代码和资源）用 AGPL-3.0-or-later，见 `LICENSE`：改过的 Miyu 拿去给别人用，包括只在网上提供服务，要把改过的源码交给用它的人。

给别人照着写东西用的这几处用 Apache-2.0（`LICENSES/Apache-2.0.txt`）：`docs/authors/`（写软件包、扩展、脚本、技能、MCP 服务的说明、模板和例子）、`docs/reference/`（协议参考）、`resources/packages/miyu-base-authoring/`（教 AI 写软件包、技能的那两份技能）。开发包 `miyu-sdk` 也用 Apache-2.0。

只经协议和 Miyu 说话的程序是独立的作品：扩展、软件包、脚本、MCP 服务器、另写的终端或网页，用什么许可证都行，AGPL 管不到它们。

不是这个仓库自己写的东西照它们原来的许可证，AGPL 和 Apache 只管这里自己写的部分：Rust 依赖照各自的 `license`；仓库里收的第三方数据（模型目录、分词词表这些）清单和各自的许可证见 `docs/blueprint/licenses.md`「资源里的第三方数据」；发行包里带的字体、ONNX Runtime 这些照各自的许可证，随包附上原文。
```

### 怎么走：门禁的「许可证」

1. 对发布的四个平台各跑一次 `cargo metadata --format-version 1 --locked --filter-platform <平台>`：`x86_64-unknown-linux-gnu`、`aarch64-unknown-linux-gnu`、`aarch64-apple-darwin`、`x86_64-pc-windows-msvc`（`12-进程形态与分发.md` R11）。取依赖图里用得到的包，去掉工作区自己的。
2. 每个包读它的 `license`，照 SPDX 表达式算：
   - `OR` 连着的，有一个能用就行；老写法里的 `/` 也当 `OR`。
   - `AND` 连着的，每个都要能用。
   - 括号照括号算。
   - `WITH` 后面是例外条款：认得的只有 `LLVM-exception`，不影响前面那个；别的例外当不能用。
3. 能用的：`MIT`、`Apache-2.0`、`BSD-2-Clause`、`BSD-3-Clause`、`ISC`、`Zlib`、`0BSD`、`Unicode-3.0`、`Unicode-DFS-2016`、`Unlicense`、`CC0-1.0`、`BSL-1.0`、`MPL-2.0`、`CDLA-Permissive-2.0`。都能和 GPL-3.0 合在一起发。
4. 没写 `license`、只给了许可证文件的，当不能用：报出来，人看过再定。
5. 同一个包在几个平台上都有，只报一次，写上是哪几个平台。

### 怎么走：第三方许可证清单

施工 AU-2（2026-10-30 项目主人定：做，照推荐在 xtask 里自己写、生成好提交进仓库）。MIT、BSD、Apache 都要求分发时附上版权和许可证原文；发出去的程序里编进了几百个第三方包，原文合进一份 `THIRD-PARTY-LICENSES.txt`。

1. 取包：对发布的四个平台各跑一次 `cargo metadata`（同上一节第 1 条），从工作区顺着**正常依赖**走得到的第三方包，四个平台合起来。开发依赖、构建依赖不编进程序，不算。
2. 每个包读它源码目录（`Cargo.toml` 那一层）里文件名照大写以 `LICENSE`、`LICENCE`、`COPYING`、`NOTICE`、`UNLICENSE` 开头的文件，是文件夹的读里面每一份；清单写了 `license-file` 的，那一份也读。换行统一成 `\n`，去掉末尾的空白；不是 UTF-8 的字节照替换字符读。
3. 一份都没带的：`license` 里出现的 `MIT`、`Apache-2.0`、`BSD-3-Clause` 用 `xtask/src/notices/` 下的标准原文，版权行写清单里的 `authors`（没写的写 `the <包名> authors`）。这三种以外的报出来，人看过再定。2026-10-30 有 13 个包没带文件，都是这三种。
4. 「资源里的第三方数据」那张表里随安装包发的两份，原文也并进来。
5. 原文一字不差的合成一段：一条线，用它的那几个（`<包名> <版本> (<license 原文>)`，资源数据写一句说明）一行一个，空一行，原文。段照第一个用它的排，同样的依赖图生成出来一字不差。2026-10-30 是 361 个包、240 段、约 650 KB。
6. `cargo xtask notices` 写这份文件；门禁的「许可证」一项重算一遍，和仓库里的不一样就算没过。升了依赖、加了依赖的那一步跟着重新生成，和代码进同一个提交。
7. 安装包根目录放 `LICENSE` 和这份清单（`packaging/README.md`）。头另有自己编进去的第三方包，各自照这个做法出一份，随头发。

### 出错

| 什么时候 | 报的话 |
|---|---|
| 一个包的许可证不能用 | `<包名> <版本>（<平台>、<平台>）：<license 原文> 和 AGPL-3.0-or-later 合不到一起` |
| 没写 `license` | `<包名> <版本>（<平台>）：没写 license，要人看过` |
| `cargo metadata` 跑不起来 | `跑不了 cargo metadata（<平台>）：<原话>` |
| 清单和依赖图对不上 | `THIRD-PARTY-LICENSES.txt 不是最新的，跑 cargo xtask notices` |
| 仓库里没有清单 | `没有 THIRD-PARTY-LICENSES.txt，跑 cargo xtask notices` |
| 包没带许可证文件，`license` 里也没有自带原文的那几种 | `<包名> <版本>：没带许可证文件，<license 原文> 也没有自带的原文，要人看过` |
| 许可证文件是空的 | `<包名> <版本>：<文件>：是空的` |

### 守着它的

| 测试 | 守哪几条 |
|---|---|
| `xtask/src/licenses/tests.rs` | 表达式：`OR`、`AND`、`WITH`、括号、`/`；能用的、不能用的、没写的；同一个包几个平台只报一次；报的那一句 |
| `xtask/src/notices/tests.rs` | 清单只算正常依赖走得到的；许可证文件、文件夹、`license-file` 都读，换行统一；没带文件的用标准原文、版权行写作者；认不出的、空的报出来；一样的原文一段、顺序不变 |

### 依赖记录

- 施工 K-5 给 `miyu-extract`（新 crate，知识库抽字的小程序，核心照子进程拉起，不编进核心）加了 `pdf-extract` 0.12（MIT，PDF 一页一页抽字）、`html2text` 0.17（MIT，网页换成文字）：知识库认 PDF、网页（`knowledge.md`「怎么走」第二条）。两棵树一百个上下的包（`lopdf`、`adobe-cmap-parser`、`html5ever`、`markup5ever` 这些），都是 MIT、Apache-2.0、ISC、BSD 这几种，都在能用的名单里，门禁过了。试过的 `pdf_oxide` 最新版自己编不过，没用。
- 施工 R-5 上给 `miyu-embed`（新 crate，本机 embedding 的小程序，`recall.md` 第四条）加了 `ort` 2.0.0-rc.13（MIT OR Apache-2.0，钉死版本，关掉默认功能只开 `std`、`download-binaries`、`tls-rustls`），它带进来的 `ort-sys`（MIT OR Apache-2.0）、`ndarray` 0.17、`matrixmultiply`、`num-complex`、`num-integer`、`rawpointer`（MIT 或 Apache-2.0）；`ort-sys` 编译时下预编好的 ONNX Runtime 用的 `ureq`、`ureq-proto`、`utf8-zero`（MIT OR Apache-2.0）、`lzma-rust2`（Apache-2.0）、`hmac-sha256`（ISC）、`socks`（MIT/Apache-2.0）、`byteorder`（Unlicense OR MIT）、`winapi` 和它的两个 `*-pc-windows-gnu`（MIT/Apache-2.0），`rustls` 那一串原来就有。分词加了 `unicode-general-category` 1.1（Apache-2.0，没有别的依赖）；`toml_edit` 多开 `serde`，带进来 `serde_spanned`（MIT OR Apache-2.0）。都在能用的名单里，门禁过了。
- 施工 R-5 六补给 `miyu-embed` 加了 `fancy-regex` 0.16（MIT）：字节级 BPE 切段的正则要往前看，`regex` 不支持；它用的 `regex-automata`、`regex-syntax` 原来就有，多带进来 `bit-set`、`bit-vec`（MIT OR Apache-2.0）。
  - ONNX Runtime 本身（MIT，微软）由 `ort-sys` 编译时从 pyke 的地址下预编好的静态库，静态链接进 `miyu-embed`，主程序不带：编这个 crate 要联网，没网的机器设 `ORT_LIB_PATH` 指到自己放好的库（`ort-sys` 的 `build/vars.rs`）。ONNX Runtime 静态库里还有它自己的第三方代码（protobuf、abseil、Eigen 等，BSD、Apache-2.0、MPL-2.0 这几种，都能和 GPL-3.0 合在一起发），发行包带上 `miyu-embed` 时随它发 ONNX Runtime 的 `ThirdPartyNotices.txt`（随发布那一步）。
- 施工 S-7 下把 `miyu-embed` 拆进自己的仓库 `miyu-agent/miyu-embed`：上面 R-5 上、六补给它加的 `ort`、`fancy-regex` 和它们带进来的包、ONNX Runtime 的静态库，连同 bge 词表、granite 测试数据这两份第三方的测试文件（许可证原文跟着搬），都随它走，核心的依赖图和仓库里不再有。核心测试用的手造小模型（`crates/miyu-session/tests/fixtures/embed-tiny/`）是自己造的，没有第三方的东西。
- 施工 W-10 给 `miyu-web` 直接加了 `getrandom` 0.4（票据的 32 个随机字节）、`base64` 0.22（核心一块块给的内容是 base64）：两个本来就在依赖图里，和别的 crate 用的同一份，没给图里添新的第三方包。门禁过了。
- 施工 W-9 给 `miyu-web`（新 crate，网页软件，`web-ui.md`）直接用了 `hyper` 1（MIT，开 `server`、`http1`：只听本机的 HTTP 端口）、`hyper-util`（开 `tokio`）、`http-body-util`、`futures-util`、`tokio`、`serde`、`serde_json`、`tracing`（都原来就在依赖图里）；新加 `tokio-tungstenite` 0.29（MIT，关掉默认功能只开 `handshake`：WebSocket 一帧一帧收发），它带进来的 `tungstenite`（MIT OR Apache-2.0）、`sha1` 0.10 和它的 `digest`、`block-buffer`、`crypto-common`、`generic-array`、`cpufeatures`（MIT 或 Apache-2.0）、`data-encoding`（MIT）、`httpdate`（MIT OR Apache-2.0）、`rand` 0.9、`rand_chacha`、`rand_core`、`ppv-lite86`、`zerocopy`（MIT 或 Apache-2.0，`zerocopy` 另可选 BSD-2-Clause）、`getrandom` 0.3（MIT OR Apache-2.0）。`getrandom` 0.3 在 wasm、UEFI 上才要的 `wasip2`、`wit-bindgen`、`r-efi` 不在发布的四个平台的依赖图里。都在能用的名单里，门禁过了。
- 施工 W-8 给 `miyu-store` 加了 `argon2` 0.6（MIT OR Apache-2.0，网页登录的密码哈希，`web-module.md`「怎么走」第一条），关掉默认功能、只开 `alloc`、`password-hash`，盐由原来就有的 `getrandom` 给；它带进来的 `password-hash`、`phc`、`base64ct`、`blake2`、`ctutils`、`cmov`（都是 MIT OR Apache-2.0）。都在能用的名单里。开发、测试编的也给 `argon2`、`blake2` 开优化（工作区 `Cargo.toml` 的 `[profile.dev.package.*]`），不然算一次要零点几秒。
- 施工 W-7 给 `miyu-net`（新 crate，可选软件包 `net`，默认打开）直接用了 `hyper-util` 0.1（MIT，开 `client-proxy`：判这一跳走不走代理，reqwest 自己照环境变量走代理用的就是它的 `Matcher`）、`http` 1（MIT OR Apache-2.0，`Matcher` 收的地址类型）；`reqwest`、`tokio`、`serde`、`serde_json`、`tracing` 照 `miyu-http` 的写法。这几个本来就在依赖图里（reqwest 带进来的），没给图里添新的第三方包。门禁过了。
- 施工 8-5 补（按 `Ctrl+C` 也把回显开回来）删了 `miyu-cli` 的 `rpassword`、它带进来的 `rtoolbox`：改成自己管终端的设置，图里少了这两个包。`miyu-cli` 多开了几个已经在图里的包的功能，没给图里添新的第三方包：`rustix`（本来就在 `miyu-basesystem`、`miyu-fs`、`miyu-ipc` 用着）开 `termios`（读写终端设置）、`pty`（只在 `dev-dependencies`，测试开真的伪终端）两个功能；`windows-sys`（本来就在别的 crate 用着）给 `miyu-cli` 直接加一条依赖、开 `Win32_System_Console`（控制台模式的读写）；`tracing`（本来就在依赖图里）给 `miyu-cli` 直接加一条依赖（终端/控制台设置写不回去时记一行）。都在能用的名单里，门禁过了。
- 施工 W-4 给 `miyu-mermaid`（新 crate，可选软件包 `mermaid`，默认打开）加了 `mermaid-rs-renderer` 0.3.1（MIT，mermaid 源码画成 SVG，关掉它默认的 `cli`、`png` 两个功能，只要出 SVG 的那一半，和它一起进来的 `clap`、`resvg`、`usvg` 都不进依赖图）；它带进来的 `anyhow`（MIT OR Apache-2.0）、`fontdb` 0.23（MIT，量字的宽要用；`miyu-mermaid` 自己也直接用它独立探一次字体库找不找得到字）、`json5`（MIT）、`once_cell`（MIT OR Apache-2.0）、`regex`（MIT OR Apache-2.0）、`serde`、`serde_json`（都在依赖图里）、`thiserror`（MIT OR Apache-2.0）、`ttf-parser`（MIT OR Apache-2.0）。`fontdb` 在 Linux 上默认功能还带来 `fontconfig-parser`（MIT，依赖 `roxmltree`，MIT OR Apache-2.0）、`memmap2`（MIT OR Apache-2.0）、`slotmap`（Zlib，依赖 `version_check`，MIT OR Apache-2.0）、`tinyvec`（Zlib OR Apache-2.0 OR MIT）、`log`（MIT OR Apache-2.0）；macOS、Windows 上不编进 `fontconfig-parser`（它只在 `cfg(all(unix, not(macos), not(android)))` 下才是依赖）。都在能用的名单里，门禁过了。`sha2`、`tracing` 原来就有。
- 施工 W-2 给 `miyu-fs` 加了 `ignore` 0.4（Unlicense OR MIT，`fs.find` 建清单走目录、认 `.gitignore`）：这个包本来就在依赖图里（`miyu-basesystem` 的 `glob`、`grep`），这次只是多一个 crate 直接用它，没给图里添新的第三方包。门禁过了。
- 施工 8-5 加了 `rpassword` 7.5（Apache-2.0，`miyu provider login` 贴 key 时关掉回显读一行），它带进来的 `rtoolbox`（Apache-2.0）；`libc`、`windows-sys` 原来就有。都在能用的名单里，门禁过了。
- 施工 8-4 加了 `notify` 8.2（CC0-1.0，监视配置文件，只开默认的 macOS FSEvents），它带进来的 `notify-types`（MIT OR Apache-2.0）、`inotify`、`inotify-sys`（ISC，Linux）、`fsevent-sys`（MIT，macOS）、`walkdir`、`same-file`（Unlicense OR MIT）、`mio`（MIT）、`bitflags`、`libc`、`log`、`windows-sys`（MIT 或 Apache-2.0，多数原来就有）。都在能用的名单里，门禁过了。
- 施工 8-2 加了 `toml_edit`（MIT OR Apache-2.0，读配置的 TOML，纯逻辑层的白名单里）和它带进来的 `toml_parser`、`toml_datetime`、`winnow`（MIT 或 Apache-2.0；`indexmap` 这些原来就有）；`sys-locale`（MIT OR Apache-2.0，系统设置里的语言）。都在能用的名单里，门禁过了。
- 施工 3-8 七补加了 `rusqlite`（MIT，会话列表的索引，`store/index.md`），开 `bundled`：`libsqlite3-sys`（MIT）自己带 SQLite 的源码编，SQLite 本身是公有领域；它带进来的 `hashlink`、`hashbrown`、`fallible-iterator`、`fallible-streaming-iterator`（MIT 或 Apache-2.0）、`foldhash`（Zlib），编的时候用的 `cc`、`pkg-config`、`vcpkg`（MIT 或 Apache-2.0）。都在能用的名单里，门禁过了。只在 wasm 上用的 `sqlite-wasm-rs`、`rsqlite-vfs` 不在发布的四个平台的依赖图里。

### 资源里的第三方数据

门禁只查 Rust 依赖。资源目录里随安装包发的第三方数据另记在这里，加一份之前先看许可证：

| 文件 | 从哪来 | 许可证 | 怎么守 |
|---|---|---|---|
| `crates/miyu-extract/data/EquivalentUnifiedIdeograph.txt` | Unicode 16.0.0 的字符数据库（施工 K-5：中文 PDF 里部首的码位换回平常的字，`knowledge.md`「怎么走」第二条第 5 款），原样，编进 `miyu-extract` | Unicode License v3（`Unicode-3.0`，在能用的名单里） | 原文放在旁边的 `EquivalentUnifiedIdeograph.LICENSE`。换 Unicode 的版本时整份替换 |
| `resources/models/models-dev.json` | models.dev 的 `api.json`（模型目录，施工 8-7） | MIT（models.dev 仓库的 `LICENSE`，2025 models.dev） | 原文放在旁边的 `models-dev.LICENSE`，跟着快照一起发。MIT 能和 GPL-3.0 合在一起发 |
| 供应商图标（不随安装包发，核心拉到缓存目录，施工 8-31） | models.dev 的 `logos/<编号>.svg`；lobe-icons 的 `@lobehub/icons-static-svg`（钉住版本） | 两份都是 MIT（models.dev 仓库的 `LICENSE`；lobe-icons 仓库的 `LICENSE`） | 只拉、只缓存，不改不发；标志是各家的商标，归各自所有者（README 写一句）。资源里只有对照表 `resources/models/logos.json`，是我们写的 |

- 施工 8-7 定的（2026-10-01，施工员确认，主会话同意照图纸带快照）：快照约 5.3 MB，进仓库压缩以后约 0.5 MB，仓库的包原来约 13.5 MB，涨不到 4%。刷新一次快照是一次替换，照「怎么刷新」（`store/resources.md`）做。

### 出处

- `12-进程形态与分发.md` R15：为什么分层，核心是 AGPL-3.0-or-later（以前是 GPL-3.0-or-later）。
