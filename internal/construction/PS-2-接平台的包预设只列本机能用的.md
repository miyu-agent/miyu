# PS-2：接平台的包预设只列本机能用的

2026-11-11 项目主人定（im 设计 13「终端里用的 im 功能」第一节第 1 条、第二节第 1 条），转给核心做的就这一处。

## 为什么

- 预设是本机会话（终端、网页）的基础层。接平台的包（清单写了 `[connection]`，比如 miyu-im）在自己平台的群、私聊里整包不过预设（施工 IM-4），那里照场所自己的开关。
- 它那些 `venues` 只写了 `private`、`group` 的工具，本机会话本来就拿不到。可预设菜单把它的全部功能、全部工具都列着：im 三项功能 22 件工具，开关开不开都没效果，和 im 后台的开关看着打架。

## 规则

只管清单里有 `[connection]` 的包，它 `[features]` 里的功能（没写 `[features]`、整个包算一个的也是）：

1. 功能下面的工具，只列本机会话拿得到的：工具目录里这件登记的 `venues` 带 `local`。没登记 `venues` 的（`Tool::venues` 是 `None`：核心自带的、没写 `Venues` 的脚本）照工具面自己的规矩，算拿得到。各类工具没写 `venues` 时的默认：扩展登记（`tools.register`/`provide`）必写，不写、写空的整件不收（`bad_tool` 的 `venues`）；包带的 MCP 服务 `[[mcp]]` 不写是只给本机（`local`）；脚本开头不写 `Venues` 是哪种会话都给；核心自带的没有这一格。
2. 一件都没有的功能不列。
3. `preset.get` 的 `sections`、`features` 两处一样（终端照 `sections` 画，网页的预设编辑页照 `features` 画）。藏掉的功能编号在功能表里是装着的，不会另列成「写了没装」。
4. 照工具目录这时的工具判：扩展从没登记过工具（没有登记缓存）、包关着的，它的这些功能都不列。
5. 不在此列：包带的 MCP 服务那几项（`feature.service`，归 MCP 一组，`venues` 由清单定）、扩展经 `feature.offer` 交的功能（交进来的就是给预设管的）。
6. 没写 `[connection]` 的包一个字节不变（工具只给私聊的也全列）。

做在 `crates/miyu-endpoint/src/presets/features.rs` 的 `listed`：拼包的功能时，接平台的包先按 `local` 筛工具，筛空了跳过；`sections.rs` 照着分组，不另判。

## 蓝图改哪几节

- `presets.md`「功能分组」加第 6 条；「守着它的」加 `preset_platform.rs` 一行。
- 参考 `personas.md` 中英：`preset.get` 的 `features` 一行补一句。
- 作者文档 `manifest.md` 中英：`[features.<编号>]` 一节补一句。

## 不做什么

- 不改会话里工具给不给：仍照 `venues`、平台前缀（IM-3）、场所开关和预设叠好的结果（`Chosen`、`crate::scripts::features`），这一步一处没碰。
- 预设里已经存着的、对藏掉的功能和工具的开关不清、不迁（没发版）：它们照旧读、照旧算进叠好的预设，只是不画；读到它们不报错，别的项照自己写的。
- 不做旧版本兼容。

## 验收

1. `crates/miyu-endpoint/tests/preset_platform.rs`：接平台的包三项功能（全是私聊和群的、混着本机的、全是本机的），菜单只列后两项，混着的只列带 `local` 的两件，`sections`、`features` 一样；不接平台、工具只给私聊的包照旧全列；预设里写着藏掉的功能和工具的开关，读得出、不另列成没装的、别的项照写的。改之前头一条、第三条红，第二条绿（钉住只管接平台的包）。
2. `cargo test -p miyu-endpoint -p miyu-config --no-fail-fast`、`cargo clippy --workspace --all-targets -- -D warnings`（加 `--target x86_64-pc-windows-gnu` 再一遍）、`cargo fmt --check`、`RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps`、`cargo xtask prompts` 无差异。

## 风险

- 扩展还没登记过工具时它的功能整组看不见，登记上以后才出现。im 装好跑起来一次就有登记缓存，之后核心一起来就照缓存先登记，平时看不到这个空档。
