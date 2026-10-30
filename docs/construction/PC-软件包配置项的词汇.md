# PC：软件包配置项的词汇

设计 `53-软件包配置项的词汇.md`（2026-10-31 项目主人定：四档都做，现在就做）。核心三步，各一个提交、各过门禁；头（网页、终端）照 `config.schema` 画，每一步合了主会话把形状告诉两个头和 miyu-agent-41。

## PC-1：选项带显示名、多行、小数、滑块（设计第一、三节）

### 目的

- `choices` 也收表的数组：`value` 必写，`name`、`description` 可以不写；`config.schema` 的 `options` 照连接的语言带 `name`（没写的是值）、`description`（没写的不带）。存的照旧是值。
- `text` 写 `lines = true` 是几行字：配置清单的 `Lines`，控件 `textarea`，不写 `max` 是 2000。
- `type = "number"`：小数，`min`、`max` 必写、只收整数（2026-10-31 主会话定：配置清单的小数范围是整数，精度用 `step`），配置清单的 `Float`，控件 `number`。
- `int`、`number` 的 `step`（正数，`int` 的是整数）、`slider = true`（要写 `min`、`max`）：控件 `slider`，`config.schema` 带 `step`；`step` 不写的 `int` 是 1、`number` 是范围的百分之一。
- 顺手：作者文档 `manifest.md` 中英 `layers` 那一行改成只能写 `system`（10-28 起，41 发现的）。

### 怎么做

1. `miyu-config`：`SettingKind::Option` 收 `Choice`（值、名字、说明）；`SettingKind` 多 `Lines`、`Number`；`Setting` 多 `slider`（一格多少）；`Control` 多 `Slider { step }`（步长挂在控件上，`Ui` 不加格，免得改遍核心自己那几十项）；`ItemWords` 多 `option_descriptions`。
2. `miyu-store` 的 `Human::with_packages`：选项的名字、说明并进字表。
3. 端点 `config.schema`：选项带 `description`，滑块带 `step`。
4. 交值不用改：小数是数、几行字照写的，同 PS-1 那一份。

### 验收

1. `miyu-config` 的 `settings::tests::vocabulary`：选项带名字、说明读对、拼成配置项照旧是值；选项写错（混着写、少 `value`、`value` 不是字、多格、重了、只有一个）报 `bad_choices`，名字写错报 `not_phrases`；几行字、小数、两种滑块读对、拼成配置项的类型和控件对；小数范围写小数、少写一头、反了、`step` 不是正数、默认值越界，整数的 `step` 是小数、滑块少写一头，`slider`、`lines` 不是开关，别的类型写这几格，各报对代码。
2. 端点 `package_vocabulary`：`config.schema` 的选项名字照中文、没写名字的是值、说明只在写了的带；几行字 `text` + `textarea` + 2000；小数 `float` + `slider` + `step`；整数滑块 `step` 1；不画滑块的小数 `number`、不带 `step`。值照类型收、读得回（几行字原样、小数、整数写的小数）；写错的 `check` 报；交给脚本、扩展的那一份小数是数、几行字照写的。修前两条都红（清单读不成）。
3. 作者文档里表和键都写了（`fields::tests`）。
4. 手写变异、本地门禁、Windows clippy、三平台 CI。

## PC-2：列表里每一项是一组字段（设计第二节）

### 目的

- 清单 `element = "record"`：`fields` 下每个字段一张表，写法同一项（不写 `layers`、`applies`，不能是 `list`），`title` 挑一个 `text`、`name`、`option` 字段当标题；整条不写默认值。
- 配置文件里写成表的数组 `[[<前缀>.accounts]]`（行内的 `[{ … }]` 也读），每一项照字段查，问题报到 `<键>[第几项].<字段>`：少写 `missing_field`（新加，三种语言的话），多写 `unknown_key`（整项不收、算错误），类型、写法照原来的码；最多 100 项，多了整条 `out_of_range`。
- `config.set` 整条换（对象的数组），字段不能单改；密钥字段只收引用，写明文的整条拒（`wrong_type`，2026-10-31 主会话定：照普通密钥项的规矩，不另开「收明文存密钥库」的路），配置文件、密钥库都不动。`config.get`、推送照旧给引用。
- 交值（握手、`extension.config`、`MIYU_SETTINGS_JSON`）：对象的数组，密钥是真值，没写的字段照默认值，没有默认值的不放。
- `config.schema`：`element: "record"`、`title`、`fields`（每个字段格同普通项，名字、说明照连接的语言）。

### 怎么做

1. `miyu-config`：`Kind::Record(&Record)`（字段是 `Item`，另带 `title`），`Value::Record`；`Record::problems`、`value_problems` 照字段查；`Code::MissingField`、`UnknownField`；`key::described_by` 认 `<键>[n].<字段>`（只给说问题的话用，改配置、查键照 `item_of`，字段不能单改）。
2. 读：`parse.rs` 的 `records` 认表的数组、行内数组，问题指到那一项的表头、那个字段。
3. 写：`edit/records.rs` 用 `toml_edit::DocumentMut` 换这一项：原来的块照原来每一块的位置、上面的注释换，多的接在原来最后一块后面，少了的连同注释去掉；行内的原地换；没写的新开几块，跟在所属的表（连它的子表）后面；空的写 `<名字> = []` 落在所属的表里（原来只因为子表才有、没有表头的开一个）；删掉的块、行内都去掉。一块里字段照字母先后写。`from_json_records` 读协议上的、逐字段报。
4. 清单：`SettingKind::Record`，字段照 `one()` 读（带「是字段」的标记）；`build.rs` 拼成 `Kind::Record`；字表 `ItemWords.fields`。顺手补 PC-1 漏改的 `bad_type`、`bad_element` 的话（多 `number`、`record`）。
5. 端点：`config.set` 的 record 列表照 `from_json_records`；`config.schema` 带 `title`、`fields`；`own()` 往一组字段里走一层；`unknown_secret`、`secret.list` 的 `used_by` 也看字段里的引用。

### 验收

1. `miyu-config`：`settings::tests::record`（读字段、标题、拼成的配置项；十几种写错各报对代码）；`parse::tests::records`（两种写法读成同一个值，空的也收；字段报到第几项的哪个字段、明文密钥是类型不对、少写的指到那一项的表头；不是表的、不是列表的；101 项整条报、100 项收；`described_by` 认字段、`item_of` 不认）；`edit::tests::records`（块照原位换、变多变少、空的、删掉、没写的新开、只有块没有表头的、空文件、行内的，逐字比对，留不住的写在测试说明里）。
2. 端点 `package_records`：`config.schema` 的形状；`config.set` 写成块、别的字节不动、`config.get` 读回引用、换成空的；明文密钥整条拒、文件一个字节不动、明文不落进配置文件和密钥库；写错的报到字段、超过 100 项、字段单改不收、手改写错 `check` 报；交给脚本、扩展的对象数组（真值、默认值、取不到的密钥字段不放）。四条在 main 的代码上都红，修后绿。
3. 作者文档对照（`fields::tests`）跟上；手写变异、本地门禁、Windows clippy、三平台 CI。
