# 写主题和外观

> <sub>本文档由 Anthropic 的 Claude Fable 5.2 生成。</sub>

终端界面和网页界面都是“头”（连接核心的界面程序）。这一页讲怎样给这两个头做一套自己的外观，并做成软件包发布出去，包括终端的主题、网页的主题和配色方案，以及吉祥物。读这一页需要会写 JSON，做网页的部分还需要会一点 JavaScript 和 CSS。

## 可以做的几种外观

可以做的外观有下面四样：

| 做什么 | 作用 | 在哪个头 | 清单 | 在哪里选择 |
|---|---|---|---|---|
| 终端的主题 | 框线、记号、文字的修饰和选中行的样子 | 终端 | `[theme.tui]` | 设置页 `外观` → `主题`，配置键是 `tui.look` |
| 网页的配色方案 | 整个页面的颜色，浅色、深色各一套 | 网页 | `[theme.web]` | 设置页 `通用` → `配色方案` |
| 网页的主题 | 圆角、阴影、边框、虚化、字体、图标 | 网页 | `[theme.web]` | 设置页 `通用` → `主题` |
| 终端的吉祥物 | 首页、侧边栏上那只吉祥物的长相和动作 | 终端 | `[mascot]` | 设置页 `外观` → `形象` → `吉祥物`，配置键是 `tui.mascot` |

终端的颜色不由软件包决定：设置页 `外观` → `配色方案` 里的 `配色` 只有自动、跟随头像、跟随壁纸、Tokyo Night、终端配色这几种，`/theme` 选的也是这一项。

带有 `[theme]` 的是外观包，带有 `[mascot]` 的是吉祥物包。一个外观包可以只带终端的部分、只带网页的部分，或者两样都带。

## 外观包的清单

外观包的 `package.toml` 写法如下：

```toml
[package]
version = "0.1.0"
name = { zh = "薄荷", en = "Mint" }

[theme.tui]
file = "theme.json"     # 终端主题：包文件夹里的相对路径

[theme.web]
dir = "web"             # 网页的部分：包文件夹里的相对目录
```

- `[theme.tui]`、`[theme.web]` 至少要写一张。`file`、`dir` 必须写，不能包含 `..`，也不能以 `/` 开头。
- 外观包只能写 `[package]`、`[theme]`、`[depends]`、`[recommends]`；如果写了 `protocol`、`[command]` 这类别的表，会报 `wrong_kind`。外观包没有程序、开关和配置项。
- `miyu check` 会检查终端主题文件（文件要存在、内容是 JSON 对象、大小不超过 256 KiB）和网页目录是否存在。

每个字段详细的见 [清单](../../authors/manifest.md#theme)。

## 终端的主题

终端主题是一个 JSON 文件，只需要写想换的几个字段。终端会把它盖在默认主题上：两边都是对象的字段逐层往里盖，其他类型（文字、列表）整个替换。以 `_` 开头的键是说明，终端不会读取。

默认主题是终端仓库里的 `packages/tui-theme-default/theme.json`，每个字段都带有说明，在它的基础上修改最快。可以写的几节如下：

| 节 | 作用 | 可以写的值 |
|---|---|---|
| `frame` | 框线：`input` 是输入框，`boxes` 是输入框上方弹出的框（命令列表、会话列表、`/model` 这些），`toast` 是弹出的小提示，`code` 是回答里代码块的线，`table` 是回答里的表格 | `rounded` 圆角、`plain` 直角、`double` 双线、`thick` 粗线、`ascii`（`+-\|`）、`none` 只留位置、不画线 |
| `marks` | 界面上的小符号：`pointer` 是选中行前面的符号，另外还有 `on`、`off`、`now`、`ok`、`fail`、`ellipsis`、`caret`、`gap`、`next`、`sep`、`dash`、`rule`、`quote`、`bar`、`side`、`secret`、`enter`、`swatch`、`heat`、`minus`、`task`（`todo`、`done`）、`chart`（八格）、`block`、`fill`（八格）、`ring`（五格） | 文字。`pointer`、`heat`、`task` 这类出现在行首的记号要两格宽 |
| `text` | 文字的修饰：`heading`、`title`、`strong`、`emphasis`、`link`、`quote`、`table_head`、`picked`（列表里选中的那一条）、`level`（权限级别）、`model`（底栏的模型名）、`selected`（选区） | 一个列表，元素可以是 `bold`、`italic`、`underline`、`dim`、`reversed`；空列表表示不加修饰 |
| `pick` | 列表里选中行的样子 | `bar` 表示整行铺底色；`mark` 表示不铺底色，只靠记号和强调色区分 |
| `layout` | 时间线、侧边栏、待办、整理记忆这些地方的记号 | 键和默认主题的这一节相同，只写想换的 |
| `timeline` | 转圈动画的帧 `spinner`、竖线 `line`、排队 `queued`、省略 `omitted` | 文字；`spinner` 是一个列表 |

如果 `frame`、`marks`、`text` 里写了不认识的键，整份文件都无法读取。如果文件无法读取，或者包已经不在了，终端会按照默认主题画，并提示 `主题包无法读取，已使用默认主题`。

终端还内置了一套 `ascii` 主题（框线、记号全部换成 ASCII 字符，选中行不铺底色）：`tui.look` 写成 `ascii` 就会使用它。如果没有设置过 `tui.look`，在 Linux 控制台（`TERM` 是 `linux`）里默认使用这套主题。

### 例子：方框主题

下面做一个主题包：把输入框和弹出的框改成直角，把选中行前面的记号改成箭头，并且不铺底色。

1. 建立目录 `tui-theme-square/`，在里面写 `package.toml`：

   ```toml
   [package]
   version = "0.1.0"
   name = { zh = "方框", en = "Square" }
   summary = { zh = "直角框线，选中不铺底", en = "Square frames, no highlight bar" }

   [theme.tui]
   file = "theme.json"
   ```

2. 在同一个目录里写 `theme.json`：

   ```json
   {
     "frame": { "input": "plain", "boxes": "plain", "toast": "plain" },
     "marks": { "pointer": "▶ " },
     "pick": "mark"
   }
   ```

3. 安装：`miyu pkg install ./tui-theme-square`。
4. 选择：在终端里输入 `/settings`，在 `外观` 页的 `主题` 一组里选择 `方框`；或者运行 `miyu config set tui.look tui-theme-square`。选择以后当场生效。

修改 `theme.json` 以后，再运行一次 `miyu pkg install ./tui-theme-square`，就会换上新的主题。

## 网页的主题和配色方案

在网页界面上，`配色方案` 决定颜色，`主题` 决定形状和材质。两样都由外观包 `[theme.web]` 指定的那个目录带进来。

这个目录是一个网页包，结构如下：

```text
web/
├── manifest.json     # 包的清单
├── index.js          # 导出 apply(ctx)
└── …                 # 样式、字体、图标，按相对地址取用
```

- `manifest.json`：写 `name`（`{"zh": …, "en": …}`）和 `inject`（要用到的服务，要挂东西就要写 `slots`）。`id` 由核心的包编号算出来，写什么都没有影响；网页包一律算作可选的，人可以在设置页的 `软件包` 里停用它。
- `index.js`：导出 `apply(ctx)`，在里面用 `ctx.slots.mount(<位置>, <一项>)` 把配色方案、主题挂上去。其他文件用 `new URL('<文件>', import.meta.url)` 取得。
- 脚本在网页里正常运行，没有额外的限制，和出厂的包一样。
- 安装、卸载以后当场生效；如果是升级，或者这个页面已经加载过这个包，输入框上方会提示刷新页面后生效。如果包启动不了（清单坏了、没有 `apply`、`apply` 抛出错误），设置页 `软件包` 里那一行会写明原因。

### 配色方案：挂进 `theme.palettes`

把配色方案挂进 `theme.palettes` 这个位置：

```js
ctx.slots.mount('theme.palettes', { id, name, render });
```

- `render(scheme)`：`scheme` 是 `'light'` 或 `'dark'`，函数交回 `{page, tui}` 两组颜色；如果给不出某一套，就交回 `null`，网页会改用其他一套。
- `page` 里的每个键会写成页面的 CSS 变量 `--<键>`（`_` 换成 `-`），`tui` 里的键写成 `--t-<键>`（网页里画终端样式的地方会用到）。整页的颜色变量一个都不能少，没有给出的变量就不存在。出厂的 `默认` 配色方案有哪些键，可以看网页仓库的 `web-demo/packages/theme-default/manifest.json`（设置项 `light_…`、`dark_…`；以 `t_` 开头的是 `tui` 那一组）。
- 最省事的写法是在出厂的 `默认` 配色方案上改几个颜色：它挂在同一个位置，编号是 `default`，用 `ctx.slots.list('theme.palettes')` 找到它，再调用它的 `render` 即可。
- 设置页卡片上的小样画的是 `sidebar_bg`、`surface`、`accent`、`text`、`gold` 这几个颜色。

### 主题：挂进 `skin.packs`

把主题挂进 `skin.packs` 这个位置：

```js
ctx.slots.mount('skin.packs', { id, name, tokens, css, icons, palette });
```

| 字段 | 说明 |
|---|---|
| `id` | 以小写字母开头，只能包含小写字母、数字和 `-`。 |
| `name` | 设置页下拉列表里显示的名字。 |
| `tokens` | 变量名到值的对应表。值是一段文字，或者浅色、深色各一份 `{"light", "dark"}`。变量名只能用网页认识的（见下文），或者以 `--skin-` 开头的自己的辅助变量。 |
| `css` | 可选：样式文件的地址（规则见下文）。 |
| `icons` | 可选：图标表的地址，写法和网页的 `resources/lucide.json` 相同；只认 `path`、`circle`、`rect`、`line`、`polyline`、`polygon`、`ellipse`、`g` 这几种图形和画图用的属性。 |
| `palette` | 可选：推荐的配色方案的编号。人选择这个主题时会顺带换上这个配色方案，之后仍然可以另选。 |

网页认识的变量在网页仓库的 `web-demo/styles/tokens.css` 里，分为两层：

- 形状的总开关：`--round`（圆角的倍数，1 是出厂值，0 表示全部直角）、`--blur`（毛玻璃虚化的倍数，0 表示不虚化）、`--text-scale`（字号的倍数）、`--space`（间距的倍数）、`--motion-scale`（动效时长的倍数，0 表示没有动效）、`--font-ui`（界面上的字体）、`--font-read`（阅读用的字体）、`--font-mono`（等宽字体）、`--font-code`（回答里代码的字体）。
- 每种材质一组变量，出厂时不定义，写了哪一组，哪一类元素的样子就会换掉。材质有：大块的区域、回答、时间线、动效、图标、输入框、发送按钮、你的话、卡片、代码块、浮起来的小框、弹窗、弹窗底下那一层、能选的一行、透明的图标按钮、按钮、主按钮、写字的框、小标签、开关的槽、开关的圆钮、分段切换的槽、分段切换选中的那一格、左栏。每组的每个变量名和说明都写在那个文件开头的注释里。

颜色只写配色方案里的颜色变量（例如 `var(--accent)`）和 `color-mix(…)`，不要写 `#rrggbb` 这样的颜色值。这样人选择 `跟随人格`、`跟随壁纸` 时，你的主题会跟着换颜色。

**样式文件**（`css`）用来处理变量做不到的地方，规则如下：

- 每条规则都以 `:root[data-skin="<你的 id>"]` 开头。
- 只用部件名 `[data-part="<名字>"]` 选取元素，名字以网页仓库的 `web-demo/resources/parts.json` 为准；状态写成 `[data-state="<状态>"]`。不写类名和 `#编号`。
- 可以带 `@font-face`、`@media`、`@supports`；自己的动画名以 `skin-<你的 id>-` 开头。
- 只要有一条规则不守规定，整份样式都不会使用；部件名不认识的那一条规则不起作用。不认的原因会写在浏览器开发者工具的控制台里（`皮肤 <id> 不认：…`）。

可以照着抄的完整例子是网页仓库的 `web-demo/packages/skin-pixel/`（`像素风` 主题，带有样式、像素字体和图标）。

### 例子：薄荷

下面做一个外观包，只换强调色，并把圆角减半。

1. 建立目录：

   ```text
   webui-theme-mint/
   ├── package.toml
   └── web/
       ├── manifest.json
       └── index.js
   ```

2. 写 `package.toml`：

   ```toml
   [package]
   version = "0.1.0"
   name = { zh = "薄荷", en = "Mint" }
   summary = { zh = "绿色的强调色，圆角小一半", en = "Green accent, half-size corners" }

   [theme.web]
   dir = "web"
   ```

3. 写 `web/manifest.json`：

   ```json
   {
     "id": "webui-theme-mint",
     "name": { "zh": "薄荷", "en": "Mint" },
     "inject": ["slots"]
   }
   ```

4. 写 `web/index.js`：

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

5. 安装：`miyu pkg install ./webui-theme-mint`。已经打开的网页会当场加载它。
6. 选择：在网页里输入 `/settings`，在 `通用` 页最上面 `主题` 一组的 `主题` 下拉列表里选择 `薄荷`；`配色方案` 一组里会多出一张 `薄荷` 卡片，并跟着被选上。
7. 查看效果：输入 `/gallery` 打开样张。样张是一个摆好了回答和时间线各种条目的会话（在里面不能发送消息），用来查看换上以后的样子。

## 吉祥物

**终端**：吉祥物包只能写 `[package]`、`[mascot]`（另外可以写 `[depends]`、`[recommends]`），例如：

```toml
[package]
name = { zh = "布丁", en = "Pudding" }

[mascot]
model = "mascot.json"
```

- `model` 是一个 JSON 对象，不超过 256 KiB，会盖在终端内置的那一只吉祥物上，内置的定义在终端仓库的 `resources/mascot.json`。每个字段都可以不写。可以写的字段有：`cols`、`rows`、`radius`、`center_row`、`ramp`、`light`、`ambient`、`shapes`、`face`、`mouth`、`idle`、`gaze`、`colors`；其中 `face`、`mouth`、`idle`、`gaze` 逐个字段地盖，其他的整个替换。如果写了其他字段，或者超出上限（`cols` 1 到 36，`rows` 1 到 18，`shapes` 1 到 32 个），整份文件都不会使用，终端会提示 `吉祥物包无法读取，已使用内置吉祥物`。
- 安装以后，在设置页 `外观` → `形象` → `吉祥物` 里选择，或者运行 `miyu config set tui.mascot <包的编号>`。
- 首页、侧边栏上画不画吉祥物由另外两项开关决定，出厂时都是关闭的：同一组里的 `首页形象` 和 `侧边栏形象`。如果会话的人格设置了头像，画出来的是头像。
- Miyu 不带出厂的吉祥物包。

**网页**：网页的吉祥物在设置页 `通用` → `主题` 一组的 `吉祥物` 里选择，出厂带有 `Deniro`。外观包也可以往网页里挂吉祥物（位置是 `mascot.characters`），但它要实现的接口还没有写成文档。

## 起名和发布

起名和发布的方法如下：

- 编号的规则和其他软件包相同（见 [写一个软件包](../../authors/README.md)）。软件源里已有的网页外观包都叫 `webui-theme-<名字>`，终端出厂的默认主题包叫 `tui-theme-default`；你的包也按这个样子起名，一看就知道是给哪个头用的。
- 打包和发布到软件源的方法见 [写一个软件包](../../authors/README.md#发到软件源)。

## 发布前的检查

发布之前，在终端和网页上各检查一遍下面几项。

终端：

- 深浅两套：把 `外观` → `配色方案` 里的 `明暗` 分别切成深色、浅色，各看一遍。
- 色深：分别在 `MIYU_COLOR=256`、`MIYU_COLOR=16`、`NO_COLOR=1` 下打开一次终端界面，看框线和选中行还能不能认出来。`MIYU_COLOR` 还可以写 `truecolor`、`none`。
- 图标：`/icons` 可以在 Nerd Font 图标和普通字符之间切换，两套都要看。
- 如果换了记号：检查宽度对不对（行首的记号要两格宽），并在你常用的几个终端里看有没有错位。

网页：

- 把 `配色方案` 里的 `明暗` 分别切成浅色、深色，各看一遍；配色方案选 `跟随人格`、`跟随壁纸` 时，你的主题颜色要跟着变化。
- 在 Firefox 和 Chromium 里都要看一遍：网页有几处是按两种浏览器的差别分别调整过的。
- 打开浏览器的开发者工具，确认控制台里没有 `皮肤 … 不认` 这类信息，设置页 `软件包` 里你的包没有写故障。
