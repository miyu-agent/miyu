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
