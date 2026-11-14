# 写主题和外观

> <sub>本文档由 Anthropic 的 Claude Fable 5.2 生成。</sub>

给终端、网页做一套自己的样子，做成软件包发出去：终端的主题、网页的主题和配色方案、吉祥物。读者要会写 JSON，做网页的部分还要会一点 JavaScript 和 CSS。

## 能做哪几样

| 做什么 | 管什么 | 在哪个头 | 清单 | 人在哪选 |
|---|---|---|---|---|
| 终端的主题 | 框线、记号、字的修饰、选中的样子 | 终端 | `[theme.tui]` | 设置页「外观」→「主题」，配置键 `tui.look` |
| 网页的配色方案 | 整页的颜色，浅色、深色各一套 | 网页 | `[theme.web]` | 设置页「通用」→「配色方案」 |
| 网页的主题 | 圆角、阴影、边框、虚化、字体、图标 | 网页 | `[theme.web]` | 设置页「通用」→「主题」 |
| 终端的吉祥物 | 首页、侧边栏上那只的长相和动作 | 终端 | `[mascot]` | 设置页「外观」→「形象」→「吉祥物」，配置键 `tui.mascot` |

终端的颜色不归软件包管：设置页「外观」→「配色方案」里的「配色」只有自动、跟随头像、跟随壁纸、Tokyo Night、终端配色几种，`/theme` 也是选它。

带 `[theme]` 的是外观包，带 `[mascot]` 的是吉祥物包。一个外观包可以只带终端的、只带网页的，或者两样都带。

## 外观包的清单

```toml
[package]
version = "0.1.0"
name = { zh = "薄荷", en = "Mint" }

[theme.tui]
file = "theme.json"     # 终端主题：包文件夹里的相对路径

[theme.web]
dir = "web"             # 网页的部分：包文件夹里的相对目录
```

- `[theme.tui]`、`[theme.web]` 至少写一张。`file`、`dir` 必写，不能有 `..`、不能以 `/` 开头。
- 外观包只能写 `[package]`、`[theme]`、`[depends]`、`[recommends]`，写了 `protocol`、`[command]` 这类别的表报 `wrong_kind`。没有程序、开关、配置项。
- `miyu check` 查终端主题文件（要在、是 JSON 对象、不超过 256 KiB）和网页目录在不在。

每一格见 [清单](../authors/manifest.md#theme)。

## 终端的主题

一个 JSON 文件，只写想换的几格。终端把它盖在默认主题上：两边都是对象的往里一层层盖，别的（字、列表）整个换掉。以 `_` 打头的键是说明，终端不读。

默认主题是终端仓库 `packages/tui-theme-default/theme.json`，每一格都带说明，照它改最快。下面是能写的几节：

| 节 | 管什么 | 能写什么 |
|---|---|---|
| `frame` | 框线：`input` 输入框、`boxes` 输入框上面开的框（命令列表、会话列表、`/model` 这些）、`toast` 弹出的小提示、`code` 回答里代码块的线、`table` 回答里的表格 | `rounded` 圆角、`plain` 直角、`double` 双线、`thick` 粗线、`ascii`（`+-\|`）、`none` 只留位置不画线 |
| `marks` | 界面上的小符号：`pointer` 选中行前面、`on`、`off`、`now`、`ok`、`fail`、`ellipsis`、`caret`、`gap`、`next`、`sep`、`dash`、`rule`、`quote`、`bar`、`side`、`secret`、`enter`、`swatch`、`heat`、`minus`、`task`（`todo`、`done`）、`chart`（八格）、`block`、`fill`（八格）、`ring`（五格） | 字。`pointer`、`heat`、`task` 这类行首的记号要两格宽 |
| `text` | 字的修饰：`heading`、`title`、`strong`、`emphasis`、`link`、`quote`、`table_head`、`picked`（列表里选中的那条）、`level`（权限级别）、`model`（底栏的模型名）、`selected`（选区） | 列表，元素是 `bold`、`italic`、`underline`、`dim`、`reversed`；空列表是不加 |
| `pick` | 列表里选中的样子 | `bar` 整行铺底色；`mark` 不铺底，只靠记号和强调色认 |
| `layout` | 时间线、侧边栏、待办、整理记忆这些地方的记号 | 键同默认主题这一节，只写想换的 |
| `timeline` | 转圈的帧 `spinner`、竖线 `line`、排队 `queued`、省略 `omitted` | 字；`spinner` 是列表 |

`frame`、`marks`、`text` 里写了不认识的键，整份读不成。读不成、包没了的，终端照默认主题画，提示「主题包无法读取，已使用默认主题」。

终端还内置一套 `ascii`（框线、记号全换成 ASCII，选中不铺底）：`tui.look` 写 `ascii` 就是它。没设过 `tui.look` 的，在 Linux 控制台（`TERM` 是 `linux`）里默认用它。

### 例子：方框主题

一个把输入框和弹出的框改成直角、选中行改成箭头、不铺底的主题包。

1. 建目录 `tui-theme-square/`，写 `package.toml`：

   ```toml
   [package]
   version = "0.1.0"
   name = { zh = "方框", en = "Square" }
   summary = { zh = "直角框线，选中不铺底", en = "Square frames, no highlight bar" }

   [theme.tui]
   file = "theme.json"
   ```

2. 同目录写 `theme.json`：

   ```json
   {
     "frame": { "input": "plain", "boxes": "plain", "toast": "plain" },
     "marks": { "pointer": "▶ " },
     "pick": "mark"
   }
   ```

3. 装：`miyu pkg install ./tui-theme-square`。
4. 选：终端里敲 `/settings`，「外观」页「主题」一组选「方框」；或者 `miyu config set tui.look tui-theme-square`。当场换上。

改了 `theme.json` 再 `miyu pkg install ./tui-theme-square` 一次就换上新的。

## 网页的主题和配色方案

网页上，「配色方案」管颜色，「主题」管形状和材质。两样都由外观包 `[theme.web]` 那个目录带进来。

那个目录是一个网页包：

```text
web/
├── manifest.json     # 包的清单
├── index.js          # 导出 apply(ctx)
└── …                 # 样式、字体、图标，照相对地址取
```

- `manifest.json`：`name`（`{"zh": …, "en": …}`）、`inject`（要用的服务，挂东西要写 `slots`）。`id` 照核心的包编号算，写什么都一样；包一律算可选的，人能在设置页「软件包」里停用。
- `index.js`：导出 `apply(ctx)`。在里面用 `ctx.slots.mount(<位置>, <一项>)` 把配色方案、主题挂上去。别的文件用 `new URL('<文件>', import.meta.url)` 取。
- 脚本在网页里照常跑，不另加限制，和出厂的包一样。
- 装上、卸掉当场生效；升级的、这一页加载过的，输入框上面提示刷新页面后生效。起不来的（清单坏了、没有 `apply`、`apply` 抛错），设置页「软件包」那一行写原因。

### 配色方案：挂进 `theme.palettes`

```js
ctx.slots.mount('theme.palettes', { id, name, render });
```

- `render(scheme)`：`scheme` 是 `'light'` 或 `'dark'`，交回 `{page, tui}` 两组颜色；交不出的交 `null`，网页用别的一套。
- `page` 里每个键写成页面的 CSS 变量 `--<键>`（`_` 换成 `-`），`tui` 里的写成 `--t-<键>`（网页里画终端样式的地方用）。整页的颜色变量一个不能少，没给的就是没有。出厂「默认」那一套有哪些键，看网页仓库的 `web-demo/packages/theme-default/manifest.json`（设置项 `light_…`、`dark_…`；`t_` 打头的是 `tui` 那一组）。
- 最省事的写法是照出厂「默认」那一套改几个：它挂在同一个位置、编号 `default`，用 `ctx.slots.list('theme.palettes')` 找到它、调它的 `render`。
- 设置页的卡片小样画 `sidebar_bg`、`surface`、`accent`、`text`、`gold` 这几个。

### 主题：挂进 `skin.packs`

```js
ctx.slots.mount('skin.packs', { id, name, tokens, css, icons, palette });
```

| 格 | 说明 |
|---|---|
| `id` | 小写字母开头，只有小写字母、数字、`-` |
| `name` | 设置页下拉里的名字 |
| `tokens` | 变量名到值。值是一段字，或者浅色、深色各一份 `{"light", "dark"}`。名字只能用网页认的（下面），或者 `--skin-` 打头的自己的辅助变量 |
| `css` | 可选：样式文件的地址（规矩见下面） |
| `icons` | 可选：图标表的地址，写法同网页的 `resources/lucide.json`；只认 `path`、`circle`、`rect`、`line`、`polyline`、`polygon`、`ellipse`、`g` 几种图形和画图的属性 |
| `palette` | 可选：推荐的配色方案的编号。人选这个主题时顺带换上它，之后还能另选 |

网页认的变量在网页仓库 `web-demo/styles/tokens.css`，分两层：

- 形状的总开关：`--round`（圆角的倍数，1 是出厂，0 是全直角）、`--blur`（毛玻璃虚化的倍数，0 不虚化）、`--text-scale`（字号的倍数）、`--space`（间距的倍数）、`--motion-scale`（动效时长的倍数，0 是不动）、`--font-ui`（界面上的字）、`--font-read`（读的字）、`--font-mono`（等宽）、`--font-code`（回答里的代码）。
- 一种材质一组，出厂不定义，写了哪个哪一类就换：大块的区域、回答、时间线、动效、图标、输入框、发送按钮、你的话、卡片、代码块、浮起来的小框、弹窗、弹窗底下那一层、能选的一行、透明的图标按钮、按钮、主按钮、写字的框、小标签、开关的槽、开关的圆钮、分段切换的槽、分段切换选中的那一格、左栏。每组的每个变量名和说明写在那个文件开头的注释里。

颜色只写配色方案的颜色变量（`var(--accent)` 这类）和 `color-mix(…)`，不写 `#rrggbb` 这样的颜色。这样人选「跟随人格」「跟随壁纸」时，你的主题跟着换色。

**样式文件**（`css`）给变量做不到的地方：

- 每条规则都以 `:root[data-skin="<你的 id>"]` 开头。
- 只用部件名 `[data-part="<名字>"]` 挑元素，名字照网页仓库的 `web-demo/resources/parts.json`；状态写 `[data-state="<状态>"]`。不写类名、`#编号`。
- 能带 `@font-face`、`@media`、`@supports`；自己的动画名以 `skin-<你的 id>-` 开头。
- 有一条不守规矩的整份样式不用；部件名不认识的那一条不起作用。不认的原因写在浏览器的开发者工具控制台里（「皮肤 <id> 不认：…」）。

照着抄的完整例子：网页仓库的 `web-demo/packages/skin-pixel/`（「像素风」，带样式、像素字体、图标）。

### 例子：薄荷

一个只换强调色、把圆角减半的外观包。

1. 建目录：

   ```text
   webui-theme-mint/
   ├── package.toml
   └── web/
       ├── manifest.json
       └── index.js
   ```

2. `package.toml`：

   ```toml
   [package]
   version = "0.1.0"
   name = { zh = "薄荷", en = "Mint" }
   summary = { zh = "绿色的强调色，圆角小一半", en = "Green accent, half-size corners" }

   [theme.web]
   dir = "web"
   ```

3. `web/manifest.json`：

   ```json
   {
     "id": "webui-theme-mint",
     "name": { "zh": "薄荷", "en": "Mint" },
     "inject": ["slots"]
   }
   ```

4. `web/index.js`：

   ```js
   // 强调色：浅色、深色各一个
   const ACCENT = { light: '#1f7a5c', dark: '#6fd3ae' };

   export function apply(ctx) {
     // 配色方案：照出厂「默认」那一套，只换强调色
     ctx.slots.mount('theme.palettes', {
       id: 'mint',
       name: '薄荷',
       render: (scheme) => {
         const base = ctx.slots.list('theme.palettes').find((p) => p.id === 'default')?.render(scheme);
         return base ? { page: { ...base.page, accent: ACCENT[scheme] }, tui: base.tui } : null;
       },
     });
     // 主题：圆角减半；选它时顺带换上上面的配色方案
     ctx.slots.mount('skin.packs', { id: 'mint', name: '薄荷', tokens: { '--round': '0.5' }, palette: 'mint' });
   }
   ```

5. 装：`miyu pkg install ./webui-theme-mint`。开着的网页当场加载它。
6. 选：网页里敲 `/settings`，「通用」页最上面，「主题」一组的「主题」下拉选「薄荷」；「配色方案」一组多了一张「薄荷」卡片，跟着选上。
7. 看：敲 `/gallery` 打开样张：一个摆好了回答、时间线各种条目的会话（在里面发不出话），看换上以后的样子。

## 吉祥物

**终端**：吉祥物包只写 `[package]`、`[mascot]`（另可写 `[depends]`、`[recommends]`）：

```toml
[package]
name = { zh = "布丁", en = "Pudding" }

[mascot]
model = "mascot.json"
```

- `model` 是一个 JSON 对象，不超过 256 KiB，盖在终端内置的那一只上：终端仓库的 `resources/mascot.json`。每一格都可以不写。能写的格：`cols`、`rows`、`radius`、`center_row`、`ramp`、`light`、`ambient`、`shapes`、`face`、`mouth`、`idle`、`gaze`、`colors`；`face`、`mouth`、`idle`、`gaze` 一格一格地盖，别的整个换。写了别的格、超了上限（`cols` 1 到 36，`rows` 1 到 18，`shapes` 1 到 32 个）的整份不用，终端提示「吉祥物包无法读取，已使用内置吉祥物」。
- 装上以后在设置页「外观」→「形象」→「吉祥物」选，或者 `miyu config set tui.mascot <包的编号>`。
- 首页、侧边栏画不画吉祥物是另两项开关，出厂都关着：同一组的「首页显示形象」「侧边栏显示形象」。会话的人格设了头像的，画的是头像。
- Miyu 不带出厂的吉祥物包。

**网页**：网页的吉祥物在设置页「通用」→「主题」一组的「吉祥物」里选，出厂有「Deniro」。外观包也能往网页挂吉祥物（位置 `mascot.characters`），它要实现的接口还没有写成文档。

## 起名和发布

- 编号的规矩同别的软件包（[写一个软件包](../authors/README.md)）。软件源里已有的网页外观包都叫 `webui-theme-<名字>`，终端出厂的默认主题包叫 `tui-theme-default`，照这个样子起，一看就知道是给哪个头的。
- 打包、发到软件源见 [写一个软件包](../authors/README.md#发到软件源)。

## 发之前看一遍

终端：

- 深浅两套：「外观」→「配色方案」的「明暗」切成深色、浅色各看一遍。
- 色深：在 `MIYU_COLOR=256`、`MIYU_COLOR=16`、`NO_COLOR=1` 下各开一次终端界面，看框线、选中的样子还认不认得出。`MIYU_COLOR` 还认 `truecolor`、`none`。
- 图标：`/icons` 在 Nerd Font 图标和普通字符之间换，两套都看。
- 换了记号的：宽度对不对（行首的两格宽），在你常用的几个终端里看有没有错位。

网页：

- 「配色方案」的「明暗」切成浅色、深色各看一遍；配色方案选「跟随人格」「跟随壁纸」时，你的主题颜色跟着变。
- 在 Firefox 和 Chromium 里都看：网页有几处是照两种浏览器的差别分开调过的。
- 打开浏览器的开发者工具，控制台里没有「皮肤 … 不认」这类话；设置页「软件包」里你的包没写故障。
