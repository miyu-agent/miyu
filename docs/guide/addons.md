# 附加软件：语义模型、OCR、渲染、表情包

> <sub>本文档由 Anthropic 的 Claude Fable 5.2 生成。</sub>

官方软件源里有四个要单独装的软件包：`embed`（内置语义模型）、`ocr`（OCR文字识别）、`render`（渲染）、`memes`（表情包）。这一页一个包一节：装了多什么、怎么装、怎么用、设置、出了问题怎么办。装包、开关、卸包的通用做法见 [软件包](packages.md)。

路径里的 `~/.miyu` 是数据根（默认 `~/.miyu`，Windows 上是 `%USERPROFILE%\.miyu`；设了 `MIYU_HOME` 的照它）。

| 编号 | 名字 | 装了多什么 | 支持的系统 |
|---|---|---|---|
| `embed` | 内置语义模型 | 记忆、知识库、表情包能照意思找 | Linux x86_64、Linux aarch64、macOS（M 系列）、Windows x86_64 |
| `ocr` | OCR文字识别 | 工具 `ocr`，命令 `miyu ocr` | Linux x86_64、macOS（M 系列）、Windows x86_64 |
| `render` | 渲染 | 工具 `render_image`，通讯平台的长回复转图 | Linux x86_64、macOS（M 系列）、Windows x86_64 |
| `memes` | 表情包 | 五件 `meme_*` 工具，后台页，通讯平台的插件 | Linux x86_64、macOS（M 系列）、Windows x86_64 |

表里没写的系统，装的时候报「不支持本机的系统」。

## 内置语义模型 embed

没装它时，找记忆、找知识库只认关键词：你问「我用什么编辑器」，记着「写代码用 Neovim」的那条对不上。装了以后照意思也找，两路合在一起排。它在本机算，不联网，记忆和对话的字不出这台机器。

装：

```sh
miyu pkg install embed
```

模型是 granite-embedding-97m-multilingual-r2（多语言，4 位量化），文件放在包里一起下，三个文件合计约 65 MB。包里的程序 `miyu-embed` 没有命令可敲，核心要用时才拉起它：一次算一句，单线程，10 分钟没有新的活就退出，下次要用再拉起。一句最多看前 512 个 token。

首次引导里也有这一步：「是否开启内置语义模型」，说明是「大幅提高记忆、知识库、表情包等内容的检索质量，但是会大幅提高内存占用，每次检索增加 CPU 开销」。选「是」当场下载、装上，写 `models.embedding = "local"`；选「否」写 `"off"`，只用关键词检索。软件源里没有这台机器能用的包时，没有这一步。

### 生效和建索引

装上、卸掉当场生效，不用重启核心。`models.embedding` 不写就是 `local`，装上就用：

| `models.embedding` | 用什么算 |
|---|---|
| `local`（默认） | 本机的 `embed` |
| `off` | 不算，只按关键词找 |
| `供应商/模型` | 那一家的模型，记忆和对话的字发给它 |

```sh
miyu config set models.embedding local
```

不用手动重建索引。向量照模型分开存，换了模型照新的补，旧的留着：搜记忆、记下新的、搜知识库的时候，在后台补还没有向量的那些。没补上的只有关键词找得到。

看补到哪了，跑 `miyu check`，末尾几行：

```text
语义模型：local:granite-embedding-97m-multilingual-r2，正在建立索引
  记忆：software-engineer 120/120
  历史对话：software-engineer 300/815
  知识库：handbook 30/42
```

每格是「建好的/总共」。没装的印「语义模型：未安装，仅按关键词搜索」，下一行提示 `miyu pkg install embed`；写了 `off` 的印「语义模型：已关闭（models.embedding = "off"），仅按关键词搜索」。换成远程模型的，末尾多一行「不自动联想：记忆、知识库（这个模型没有测过门槛，搜索照常）」：只有内置的模型会自动想起记忆和知识库，别的模型只在搜索时用到。

### 不装会怎样

- 记忆：AI 不联想，`memory_search` 只按关键词找。见 [记忆](memory.md)。
- 知识库：只按关键词找，自动想起只认标题和关键词。见 [知识库](knowledge.md)。
- 表情包：`meme_search` 只按关键词找。

出厂的「人格记忆」（`base-memory`）卸掉了的，`embed` 装着也不用。

## OCR文字识别 ocr

在本机认图片里的中文、英文，交回一行行字，从上到下。不用另装 Tesseract、Python。

```sh
miyu pkg install ocr
```

装好是关着的，确认屏后面问「现在启用 'ocr'？[Y/n]」，答是。关着时 AI 没有 `ocr` 工具，`miyu ocr` 照样能用。

模型不在包里：打开以后包里的程序在后台下，约 16 MB，从 [oar-ocr 的 Release](https://github.com/GreatV/oar-ocr/releases/tag/v0.3.0) 取，照写死的 SHA-256 核对过才用，放在 `~/.miyu/var/cache/ocr/paddleocr/`。走系统的代理设置（`HTTPS_PROXY` 这类）。用的是 PaddleOCR 的手机版模型：找字 PP-OCRv5，认字 PP-OCRv4。

### 在命令行里认

```sh
miyu ocr 截图.png    # 印出图里的字，一行一行
miyu ocr             # 看模型下好没有
```

| 写法 | 印什么 |
|---|---|
| `miyu ocr <图>` | 图里的字，一行一行。路径照当前目录算。模型没下的先下 |
| `miyu ocr` | 下好了的印 `model: ready (<模型的目录>)`；没下的印 `model: not downloaded yet (16 MB, downloaded the first time it is used)` |
| `miyu ocr -h`、`miyu ocr --help` | 用法 |

认不出的报错印在标准错误上，`miyu ocr: ` 打头，退出码 `1`。

### AI 用的工具 ocr

参数只有 `path`：图片的路径，相对工作区或者绝对路径。交回一行行字；一个字都没认出来的交回 `No text found in the image.`。一次最多等 180 秒。它只读文件，只读下照样能用。

AI 在命令里写脚本时也能用 `miyu call ocr --path <图>` 批量认，拿到 `{"lines": [{"text", "box", "score"}]}`：`box` 是这一行在原图上的左、上、右、下（像素），`score` 是 0 到 1 的把握。`miyu call` 见 [命令行](cli.md#miyu-call)。

### 认什么

- 格式：PNG、JPEG、WebP、GIF、BMP、TIFF。照文件内容认，扩展名不算数。手机照片照 EXIF 转正。
- 大小：文件不超过 50 MiB，不超过 8000 万像素。
- 字：中文、英文、数字和常用符号。横排印刷体认得最好。

认不好的：斜着拍的、竖排的、手写的，日文假名、韩文。

### 出了问题

| 看到的 | 怎么办 |
|---|---|
| `could not download https://github.com/…` | 连不上 GitHub。设好代理再试，下次调用会重下 |
| `the download's SHA-256 is …, expected …` | 下坏了或者被换了，没装上。再试一次 |
| `not an image this tool reads (PNG, JPEG, WebP, GIF, BMP, TIFF)` | 文件不是这几种图 |
| `the image is larger than 50 MiB`、`the image is too large (…)` | 图太大，先缩小或者切开 |
| `no such file: <路径>` | 路径不对。`miyu ocr` 的路径照当前目录算 |

## 渲染 render

把 Markdown 画成 PNG 图。主要给通讯平台用：回复太长时 im 把它画成图发出去，见 [接入通讯平台](heads/im.md#消息怎么显示)。在终端、网页里也能让 AI 画。

```sh
miyu pkg install render
```

装上就跑，不用打开。不用下模型。

### 能画什么

- 标题、段落、列表、引用、分割线。段落里单个换行照样换行。
- 代码块，认得出语言的上色。
- 表格，表头加粗；跨栏时续栏先重复表头。
- 公式：独占一行的 `$$…$$` 画成公式；行内的 `$…$` 换成 Unicode 写法。
- Mermaid 图：语言写 `mermaid` 的代码块画成图。节点、连线超过 200 的照代码块画。
- 链接：画成图点不了，标题后面补上 `(地址)`。图片 `![说明](地址)` 不画图，只留说明和地址。
- 夹着的 HTML：`<br>` 换行，`<b>`、`<i>`、`<s>`、`<code>` 换成样式，别的标签只留字。

一次最多 20000 个字，最多画 30 秒。长的自动分栏、分页：一页最高 5000 像素，成图接近方形，几页合起来有像素上限，超了报太长。

### AI 用的工具 render_image

参数 `markdown`（要画的字）和 `path`（工作区里的一个 `.md` 或 `.markdown` 文件）二选一。画好的图存成文件，交回每一页的路径和长宽，不会自己发给你：要看，叫 AI 用「展示给用户」（`show`）给你看。

- 本机的会话：存在 `~/.miyu/root/pictures/`。
- 通讯平台的会话：存在工作区的 `rendered/`。

文件名是 `render-<一串字>-<页>.png`。同样的字、同样的设置、同一版程序画过的，直接交回已经有的图。它要写文件，只读下用不了。

```text
把上面那张对比表画成图给我看
```

### 字体

先用系统的字体：Linux 问 fontconfig 要中文的无衬线和等宽；macOS 用 PingFang SC、Menlo 这些；Windows 用 Microsoft YaHei、Consolas 这些。设置里指定的字体没有中文的，正文换成系统里一个有中文的。一个有中文的字体都没有的，画不了，见下面「出了问题」。不下载字体。

### 设置

| 键 | 界面上叫 | 默认 | 说明 |
|---|---|---|---|
| `render.theme` | 配色 | `paper` | `paper` 纸色，`light` 近白，`dark` 深灰 |
| `render.font` | 字体 | 空 | 正文的字体族名，最长 128 字。空的照系统的无衬线 |
| `render.code_font` | 代码字体 | 空 | 代码的字体族名。空的照系统的等宽 |
| `render.font_size` | 字号（像素） | `36` | `14` 到 `56` |

改了当场生效：

```sh
miyu config set render.theme dark
miyu config set render.font "Noto Serif CJK SC"
```

### 给别的软件包用

`render` 在总线上的名字是 `render`，方法 `render.markdown`：别的包交 Markdown 和可选的配色，拿回一页页图。写包的人看 [总线](../reference/bus.md)。

`miyu render` 这条命令是核心拉起包里的程序用的，你敲了只印 `usage: miyu-render serve`。

### 出了问题

| 看到的 | 怎么办 |
|---|---|
| `no font with Chinese characters is installed; …` | 装一个中文字体：Linux 装 Noto Sans CJK（例如 `noto-fonts-cjk` 包）；Windows 装 Microsoft YaHei 或 Noto Sans CJK；macOS 看系统字体是不是被停用了 |
| `markdown is longer than 20000 characters` | 拆成几段画 |
| `took too long to draw` | 30 秒没画完，多半是图太大。拆开画 |
| `the renderer is busy` | 前面超时的还没画完，过一会儿再试 |
| `too long to draw` | 超了像素上限，拆成几段画 |
| Mermaid 图画成了代码 | 图写错了，或者节点、连线太多 |

## 表情包 memes

一个表情包库：每张图带「描述」（图上画了什么、写了什么字）和「用法」（聊天里什么时候合适）。AI 照意思从库里挑一张发，也能把聊天里看到的图存进来。

```sh
miyu pkg install memes
```

装上就跑，不用打开。库在 `~/.miyu/var/lib/memes/library.db`。显示不了图的会话里，这个包当没开。

### AI 用的五件工具

| 工具 | 界面上叫 | 做什么 | 参数 |
|---|---|---|---|
| `meme_search` | 挑选表情包 | 照意思找，交回每张的编号、描述、用法 | `query`（想表达什么，必填）、`limit`（1 到 20，默认 5） |
| `meme_send` | 发送表情包 | 单独发一张。默认发完这一轮就结束，`end` 写 `false` 接着说 | `id`（必填）、`end` |
| `meme_save` | 保存表情包 | 存一张进库：本机的文件，或者聊天里一条消息的第几张图 | `reason`（为什么存，必填）、`path`，或者 `msg` 加 `index`（从 1 数，默认 1）；`description`、`usage` |
| `meme_edit` | 编辑表情包 | 改一张的描述或用法 | `id`（必填）、`description`、`usage` |
| `meme_delete` | 删除表情包 | 停用一张，后台页里能恢复 | `id`（必填） |

`meme_save`、`meme_edit`、`meme_delete` 要写，只读下用不了。

照意思找要内置语义模型（`embed`）或者 `models.embedding` 配的远程模型，没有的只按关键词找。

库里有能发的图时，每一轮 AI 回话前掷一次骰子，中了在给 AI 的内容里加一句提醒，让它这一轮配一张表情包。概率见下面的设置。

### 往库里加图

- 你自己传：在后台页按「上传」，拖进或选一张图，写描述和用法（必填），或者按「让 AI 写」。
- 装图包：

  ```sh
  miyu pkg install memes-shorin-collection
  ```

  这是一套现成的图（38 张）。装上就进库，卸掉就从库里拿掉。图包在后台页的来源是「内容包」。
- AI 在聊天里存：叫它存（「把刚才那张存成表情包」），它调 `meme_save`。AI 自己看到了图的，自己写描述和用法；没写的，交给看图的模型（`models.vision`）写，顺带判断是不是表情包：截图、照片、文档、图表不收。没配 `models.vision` 的，AI 没自己写描述就存不了。

同一张图已经在库里的不重复存，交回原来的编号。单张超过上限的先缩小再存；动图不缩，超了不收。

### 后台页

```sh
miyu pkg page memes
```

网页设置页的「软件后台」、im 插件卡片上的「打开面板」打开的也是它。

- 顶上一排：「搜索」框；来源筛选（「全部来源」「AI 保存」「手动上传」「内容包」「其他软件」：别的软件包经总线存进来的）；「显示已停用」；「重新生成语义索引」；「上传」。
- 下面是网格，点一张看详情：大图，能改的「描述」「用法」，「来源」「收藏理由」「使用次数」「尺寸」。按钮「保存」「停用」（停用了的是「启用」）「删除」「重新描述」。
- 「重新描述」「让 AI 写」用看图的模型写好填进框里，你看过再按「保存」。没配看图模型的说「未配置看图模型」。
- 「删除」是彻底删掉，问一句「彻底删除这张表情包？」。AI 的 `meme_delete` 只是停用。

### 在通讯平台上用

表情包是 im 的一个插件，默认开着，在会话卡的「管理插件」里按群、私聊开关。关着的地方，AI 没有这五件工具，也不提醒发表情包。见 [接入通讯平台](heads/im.md#插件)。

### 设置

| 键 | 界面上叫 | 默认 | 说明 |
|---|---|---|---|
| `memes.reminder` | 自动提醒发表情包 | `true` | 关掉就不掷骰子 |
| `memes.reminder_probability` | 每次回复提醒的概率（%） | `3` | `0` 到 `100` |
| `memes.max_size_mb` | 单张上限（MiB） | `5` | `1` 到 `20` |

改了当场生效：

```sh
miyu config set memes.reminder_probability 10
```

`miyu memes` 这条命令是核心拉起包里的程序用的，你敲了只印 `usage: miyu-memes serve`。别的包经总线上的名字 `memes`（`memes.add`、`memes.search`）往库里存、在库里找，见 [总线](../reference/bus.md)。

### 出了问题

| 看到的 | 怎么办 |
|---|---|
| `Not saved: no vision model could describe it (…)` | 配 `models.vision`（见 [模型](models.md#三个用途)），或者让 AI 看过图以后自己写描述和用法 |
| `Not saved: it is a screenshot, photo, document or chart, not a sticker.` | 看图的模型判断这不是表情包。真要存，在后台页自己传 |
| `msg works only in a chat.` | `msg` 只在通讯平台的会话里能用，本机的会话用 `path` |
| 后台页「这张图缩不到能传的大小（动图不缩）」 | 换一张小一点的，或者调大 `memes.max_size_mb` |
| 后台页「看图模型不支持图片，请在设置里换一个」 | `models.vision` 配的模型收不了图，换一个 |
| 后台页「未开启语义模型」 | 装 `embed`，或者 `models.embedding` 写了 `off` |
