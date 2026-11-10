## 施工单 SG-2：运行时给选项

状态：施工中（2026-11-10）。项目主人验收语音助手设置页时提的。

### 为什么

两件都出在语音助手的设置页上。一是麦克风那一项是文字框，要人手填设备名：这台机器上有哪些设备只有包的程序运行时才知道，清单的 `option` 只能写死选项，包没法把现查的列表交给设置页。二是语音只在本机用，主人天然是 `voice:owner`（施工 VA-1），可平台接入的包信息页一律画「你在这个平台上的号」，于是出现了要人填「voice 号」的一块，头分不出哪个平台用不着填。

### 做什么

1. 清单：`type = "text"` 的项（不是 `lines` 的那种，组里的也行）可写 `choices_from = "<方法名>"`，是这个包自己用 `package.methods` 登记的方法，写法同登记的（小写字母开头，小写字母、数字、`_`、`.`、`-`，1 到 64 个）。几行字、列表的元素、一组字段的字段上写了的，名字写法不对、不是字的，清单读不成，报新的 `bad_choices_from`（三种语言的字）；别的类型不认这一格，照旧报 `unknown_key`（`packages.md`「配置项」的表）。
2. 值照旧是普通文字：核心不调那个方法、不查它登记了没有、不照列表查值（设备拔了不算错）。
3. `config.schema`：这一项带 `choices_from: {"package": "<包的编号>", "method": "<方法名>"}`；没写的项不带，字节不变（`config.md`「协议」）。
4. 头怎么用（只写进文档，核心不实现）：画成下拉，`package.call {package, method}` 取选项，包回 `{"choices": [{"value", "name"?, "description"?}]}`；调不到、回空的退回文字框；存着的值不在列表里的照样显示。
5. `package.list`：平台是 `voice` 的包，`connection` 带 `owner_builtin: true`，别的平台不带、字节不变；接几个平台的照 `platform`（第一个）算。平台名提成 `venues::VOICE`，认主人的规矩和这里用同一个，不再各写一份字面量。
6. 文档：作者 `manifest`、参考 `config`、`packages` 中英，蓝图 `packages.md`、`config.md`、`package-pages.md`。

### 不做什么

- 核心不替头去调方法、不缓存选项、不把选项并进 `options`。
- 列表的元素、一组字段的字段不收 `choices_from`：现在没有要用的包。
- 头的下拉、包里登记的方法各归各的仓库。

### 验收

- `miyu-config`（`settings/tests/choices_from.rs`）：写对的读出来（组里的、64 个字符的、默认值不照列表查），没写的没有；空的、带空白、大写或数字开头、不是 ASCII、65 个字符、不是字、几行字、文字的列表、一组字段的字段，都报 `bad_choices_from`、报在那一行；别的类型报 `unknown_key`。先红：改之前没有这一格，测试编不过。
- 集成（`tests/package_choices_from.rs`）：`config.schema` 里那一项带对的 `{package, method}`（包的编号，不是前缀），没写的项、核心自己的项都不带；不在列表里的值照样写得进、读得到；平台 `voice` 的包带 `owner_builtin`，`qq` 的不带，`["voice", "tg"]` 的照第一个算。先红：改之前带 `choices_from` 的清单读不成（两条红），`voice` 的包没有 `owner_builtin`（一条红）。
- `cargo test -p miyu-config`、`cargo test -p miyu-endpoint`、整个工作区的 clippy、`cargo fmt --check`、两个 crate 的 doc 全过。改协议，合了告诉 UI 和 im。
