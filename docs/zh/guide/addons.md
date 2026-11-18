# 附加软件：语义模型、OCR、渲染、表情包

> <sub>本文档由 Anthropic 的 Claude Fable 5.2 生成。</sub>

官方软件源里有四个需要单独安装的软件包：`embed`（内置语义模型）、`ocr`（OCR文字识别）、`render`（渲染）、`memes`（表情包）。这一页每个软件包一节，介绍安装后多了什么、怎么安装、怎么使用、有哪些设置，以及出了问题怎么处理。安装、开关、卸载软件包的通用方法见 [软件包](packages.md)。

下文路径里的 `~/.miyu` 是数据根（默认是 `~/.miyu`，Windows 上是 `%USERPROFILE%\.miyu`；如果设置了 `MIYU_HOME`，就使用它指定的目录）。

| 编号 | 名字 | 安装后多了什么 | 支持的系统 |
|---|---|---|---|
| `embed` | 内置语义模型 | 记忆、知识库、表情包可以按意思搜索。 | Linux x86_64、Linux aarch64、macOS（M 系列）、Windows x86_64 |
| `ocr` | OCR文字识别 | 工具 `ocr` 和命令 `miyu ocr`。 | Linux x86_64、macOS（M 系列）、Windows x86_64 |
| `render` | 渲染 | 工具 `render_image`，以及通讯平台上长回复转成图片。 | Linux x86_64、macOS（M 系列）、Windows x86_64 |
| `memes` | 表情包 | 五个 `meme_*` 工具、一个后台页和一个通讯平台的插件。 | Linux x86_64、macOS（M 系列）、Windows x86_64 |

在表里没有列出的系统上安装时，会报错 `不支持该系统。`。

## 内置语义模型 embed

没有安装它时，搜索记忆和知识库只按关键词匹配：例如你问「我用什么编辑器」，记着「写代码用 Neovim」的那条记忆就匹配不上。安装以后也会按意思搜索，两种结果合在一起排序。它在本机计算，不联网，记忆和对话的文字不会离开这台机器。

安装它，运行：

```sh
miyu pkg install embed
```

模型是 granite-embedding-97m-multilingual-r2（多语言，4 位量化），模型文件放在软件包里一起下载，三个文件合计约 65 MB。包里的程序 `miyu-embed` 没有可以直接运行的命令，核心需要时才会启动它：它一次计算一句，单线程运行，10 分钟没有新的任务就退出，下次需要时再启动。一句最多只看前 512 个 token。

第一次启动的新手引导里也有这一步：`是否开启内置语义模型`，说明文字是 `大幅提高记忆、知识库、表情包等内容的检索质量，但是会大幅提高内存占用，每次检索增加 CPU 开销`。选择 `是` 会当场下载并安装，并写入 `models.embedding = "local"`；选择 `否` 会写入 `"off"`，只使用关键词检索。如果软件源里没有这台机器能用的软件包，就没有这一步。

### 生效和建立索引

安装、卸载都会当场生效，不需要重启核心。`models.embedding` 没有填写时就是 `local`，安装以后就会使用：

| `models.embedding` | 计算方式 |
|---|---|
| `local`（默认） | 使用本机的 `embed`。 |
| `off` | 不计算，只按关键词搜索。 |
| `供应商/模型` | 使用那一家的模型，记忆和对话的文字会发给它。 |

```sh
miyu config set models.embedding local
```

不需要手动重建索引。向量按模型分开保存，换了模型以后会按新模型补算，旧的向量保留：搜索记忆、记下新记忆、搜索知识库时，会在后台补算还没有向量的内容。还没有补算的内容只能用关键词找到。

要查看补算的进度，运行 `miyu check`，看输出的最后几行：

```text
语义模型：local:granite-embedding-97m-multilingual-r2，正在建立索引
  记忆：software-engineer 120/120
  历史对话：software-engineer 300/815
  知识库：handbook 30/42
```

每一项的格式是 `建好的/总共`。如果没有安装，会输出 `语义模型：未安装，仅按关键词搜索`，下一行提示 `miyu pkg install embed`；如果写了 `off`，会输出 `语义模型：已关闭（models.embedding = "off"），仅按关键词搜索`。如果换成了远程模型，最后会多一行 `不自动联想：记忆、知识库（这个模型没有测过门槛，搜索照常）`：只有内置的模型会自动想起记忆和知识库，其他模型只在搜索时使用。

### 不安装的影响

- 记忆：AI 不会自动联想，`memory_search` 只按关键词搜索。见 [记忆](memory.md)。
- 知识库：只按关键词搜索，自动想起只看标题和关键词。见 [知识库](knowledge.md)。
- 表情包：`meme_search` 只按关键词搜索。

如果卸载了出厂的 `人格记忆`（`base-memory`），即使安装了 `embed` 也不会使用。

## OCR文字识别 ocr

这个软件包在本机识别图片里的中文和英文，按从上到下的顺序交回一行一行的文字，不需要另外安装 Tesseract 或 Python。安装它，运行：

```sh
miyu pkg install ocr
```

安装后它默认是关着的，确认屏之后会询问 `现在启用 'ocr'？[Y/n]`，此时回答是。关着时 AI 没有 `ocr` 工具，但 `miyu ocr` 仍然可以使用。

模型不在软件包里：打开以后，包里的程序会在后台下载模型，约 16 MB，从 [oar-ocr 的 Release](https://github.com/GreatV/oar-ocr/releases/tag/v0.3.0) 获取，按照程序里写死的 SHA-256 校验通过后才使用，保存在 `~/.miyu/var/cache/ocr/paddleocr/`。下载使用系统的代理设置（`HTTPS_PROXY` 这类）。使用的是 PaddleOCR 的手机版模型：检测文字用 PP-OCRv5，识别文字用 PP-OCRv4。

### 在命令行里识别

```sh
miyu ocr 截图.png    # 输出图里的文字，一行一行
miyu ocr             # 查看模型有没有下载好
```

| 写法 | 输出 |
|---|---|
| `miyu ocr <图>` | 图里的文字，一行一行。路径相对当前目录计算。如果模型还没有下载，会先下载。 |
| `miyu ocr` | 下载好时输出 `model: ready (<模型的目录>)`；没有下载时输出 `model: not downloaded yet (16 MB, downloaded the first time it is used)`。 |
| `miyu ocr -h`、`miyu ocr --help` | 用法。 |

识别失败时，报错输出在标准错误上，以 `miyu ocr: ` 开头，退出码是 `1`。

### AI 使用的工具 ocr

这个工具只有一个参数 `path`：图片的路径，可以是相对工作区的路径，也可以是绝对路径。它交回一行一行的文字；一个字都没有识别出来时，交回 `No text found in the image.`。一次最多等待 180 秒。它只读取文件，所以在只读权限级别下也能使用。

AI 在命令里写脚本时，也可以用 `miyu call ocr --path <图>` 批量识别，得到 `{"lines": [{"text", "box", "score"}]}`：`box` 是这一行在原图上的左、上、右、下坐标（像素），`score` 是 0 到 1 之间的把握程度。`miyu call` 见 [命令行](cli.md#miyu-call)。

### 支持识别的内容

- 格式：PNG、JPEG、WebP、GIF、BMP、TIFF。按文件内容识别格式，扩展名不起作用。手机照片会按 EXIF 转正。
- 大小：文件不超过 50 MiB，并且不超过 8000 万像素。
- 文字：中文、英文、数字和常用符号。横排的印刷体识别效果最好。

识别效果不好的情况：斜着拍的图片、竖排文字、手写文字，以及日文假名和韩文。

### 常见问题

| 看到的现象 | 解决方法 |
|---|---|
| `could not download https://github.com/…` | 连不上 GitHub。设置好代理再试，下次调用时会重新下载。 |
| `the download's SHA-256 is …, expected …` | 下载的文件损坏或者被替换了，没有安装。再试一次即可。 |
| `not an image this tool reads (PNG, JPEG, WebP, GIF, BMP, TIFF)` | 文件不是这几种格式的图片。 |
| `the image is larger than 50 MiB`、`the image is too large (…)` | 图片太大，先缩小或者切开。 |
| `no such file: <路径>` | 路径不对。`miyu ocr` 的路径相对当前目录计算。 |

## 渲染 render

这个软件包把 Markdown 画成 PNG 图片，主要给通讯平台使用：回复太长时，im 会把回复画成图片发出去，见 [接入通讯平台](heads/im.md#消息的显示方式)。在终端界面、网页界面里也可以让 AI 画图。安装它，运行：

```sh
miyu pkg install render
```

安装后就会运行，不需要手动开启，也不需要下载模型。

### 支持渲染的内容

- 标题、段落、列表、引用、分割线。段落里的单个换行也会照样换行。
- 代码块，能识别出语言的会上色。
- 表格，表头加粗；表格跨栏时，续栏会先重复一遍表头。
- 公式：独占一行的 `$$…$$` 画成公式；行内的 `$…$` 换成 Unicode 写法。
- Mermaid 图：语言写成 `mermaid` 的代码块会画成图。节点和连线超过 200 个时，按代码块画。
- 链接：画成图片后无法点击，所以在链接的标题后面补上 `(地址)`。图片 `![说明](地址)` 不画出图片，只保留说明和地址。
- 夹在里面的 HTML：`<br>` 换行，`<b>`、`<i>`、`<s>`、`<code>` 换成对应的样式，其他标签只保留文字。

一次最多画 20000 个字，最多画 30 秒。长内容会自动分栏、分页：一页最高 5000 像素，成图接近方形，几页合起来有像素上限，超过上限会报错太长。

### AI 使用的工具 render_image

参数 `markdown`（要画的文字）和 `path`（工作区里的一个 `.md` 或 `.markdown` 文件）二选一。画好的图片保存成文件，工具交回每一页的路径和长宽，不会自己发给你：要看图片，叫 AI 用 `展示给用户`（`show`）给你看。

- 本机的会话：保存在 `~/.miyu/root/pictures/`。
- 通讯平台的会话：保存在工作区的 `rendered/`。

文件名是 `render-<一串字>-<页>.png`。如果同样的文字、同样的设置、同一版程序已经画过，就直接交回已经有的图片。这个工具要写文件，所以在只读权限级别下不能使用。例如可以这样对 AI 说：

```text
把上面那张对比表画成图给我看
```

### 字体

渲染优先使用系统的字体：Linux 上向 fontconfig 查询中文的无衬线字体和等宽字体；macOS 上使用 PingFang SC、Menlo 这些字体；Windows 上使用 Microsoft YaHei、Consolas 这些字体。如果设置里指定的字体不含中文，正文会换成系统里一个含中文的字体。如果系统里一个含中文的字体都没有，就无法画图，见下面的「常见问题」。渲染不会下载字体。

### 设置

| 键 | 界面上的名字 | 默认 | 说明 |
|---|---|---|---|
| `render.theme` | 配色 | `paper` | `paper` 是纸色，`light` 是近白，`dark` 是深灰。 |
| `render.font` | 字体 | 空 | 正文的字体族名，最长 128 个字。为空时使用系统的无衬线字体。 |
| `render.code_font` | 代码字体 | 空 | 代码的字体族名。为空时使用系统的等宽字体。 |
| `render.font_size` | 字号（像素） | `36` | `14` 到 `56` |

修改后当场生效，例如：

```sh
miyu config set render.theme dark
miyu config set render.font "Noto Serif CJK SC"
```

### 供其他软件包调用

`render` 在总线上的名字是 `render`，方法是 `render.markdown`：其他软件包交给它 Markdown 和可选的配色，拿回一页一页的图片。写软件包的作者可以看 [总线](../../reference/bus.md)。

`miyu render` 这条命令是给核心启动包里的程序用的，你手动运行它只会输出 `usage: miyu-render serve`。

### 常见问题

| 看到的现象 | 解决方法 |
|---|---|
| `no font with Chinese characters is installed; …` | 安装一个中文字体：Linux 上安装 Noto Sans CJK（例如 `noto-fonts-cjk` 软件包）；Windows 上安装 Microsoft YaHei 或 Noto Sans CJK；macOS 上检查系统字体是不是被停用了。 |
| `markdown is longer than 20000 characters` | 拆成几段分别画。 |
| `took too long to draw` | 30 秒内没有画完，多半是图太大。拆开画。 |
| `the renderer is busy` | 前面超时的任务还没有画完，过一会儿再试。 |
| `too long to draw` | 超过了像素上限，拆成几段画。 |
| Mermaid 图画成了代码 | 图的写法有错，或者节点、连线太多。 |

## 表情包 memes

这个软件包是一个表情包库：每张图带有 `描述`（图上画了什么、写了什么字）和 `用法`（在聊天里什么时候适合用）。AI 会按意思从库里挑一张发送，也可以把聊天里看到的图存进库里。安装它，运行：

```sh
miyu pkg install memes
```

安装后就会运行，不需要手动开启。库保存在 `~/.miyu/var/lib/memes/library.db`。在无法显示图片的会话里，这个软件包视为没有开启。

### AI 使用的五个工具

| 工具 | 界面上的名字 | 作用 | 参数 |
|---|---|---|---|
| `meme_search` | 挑选表情包 | 按意思搜索，交回每张的编号、描述和用法。 | `query`（想表达什么，必填）、`limit`（1 到 20，默认 5） |
| `meme_send` | 发送表情包 | 单独发送一张。默认发完这一轮就结束，把 `end` 写成 `false` 可以接着说话。 | `id`（必填）、`end` |
| `meme_save` | 保存表情包 | 把一张图存进库里：可以是本机的文件，也可以是聊天里某条消息的第几张图。 | `reason`（为什么存，必填）、`path`，或者 `msg` 加 `index`（从 1 数，默认 1）；`description`、`usage` |
| `meme_edit` | 编辑表情包 | 修改一张的描述或用法。 | `id`（必填）、`description`、`usage` |
| `meme_delete` | 删除表情包 | 停用一张，可以在后台页里恢复。 | `id`（必填） |

`meme_save`、`meme_edit`、`meme_delete` 需要写入，所以在只读权限级别下不能使用。

按意思搜索需要内置语义模型（`embed`）或者 `models.embedding` 配置的远程模型；没有时只按关键词搜索。

库里有可以发送的图时，AI 每一轮回复前会掷一次骰子，掷中了就在给 AI 的内容里加一句提醒，让 AI 这一轮配一张表情包。概率见下面的「设置」。

### 向库里添加图片

- 你自己上传：在后台页点击 `上传`，拖进或选择一张图，填写描述和用法（必填），或者点击 `让 AI 写`。
- 安装图包，运行：

  ```sh
  miyu pkg install memes-shorin-collection
  ```

  这是一套现成的图（38 张）。安装后这些图会进入库里，卸载后会从库里移除。图包在后台页上的来源显示为 `内容包`。
- AI 在聊天里保存：叫 AI 保存（例如「把刚才那张存成表情包」），AI 会调用 `meme_save`。如果 AI 自己看到了图，就自己写描述和用法；如果 AI 没有写，就交给看图的模型（`models.vision`）来写，同时判断这张图是不是表情包：截图、照片、文档、图表都不收。如果没有配置 `models.vision`，AI 没有自己写描述时就存不了。

同一张图已经在库里时不会重复保存，交回原来的编号。单张超过上限的图会先缩小再保存；动图不缩小，超过上限就不收。

### 后台页

打开表情包的后台页，运行：

```sh
miyu pkg page memes
```

网页界面设置页里的 `软件后台`，以及 im 插件卡片上的 `打开面板`，打开的也是这个页面。

- 顶部一排是：`搜索` 框；来源筛选（`全部来源`、`AI 保存`、`手动上传`、`内容包`、`其他软件`：其他软件包经过总线存进来的图）；`显示已停用`；`重新生成语义索引`；`上传`。
- 下面是网格，点击一张可以查看详情：大图，可以修改的 `描述`、`用法`，以及 `来源`、`收藏理由`、`使用次数`、`尺寸`。按钮有 `保存`、`停用`（已停用的图显示为 `启用`）、`删除`、`重新描述`。
- `重新描述` 和 `让 AI 写` 会用看图的模型写好内容并填进框里，你看过以后再点击 `保存`。如果没有配置看图模型，会提示 `未配置看图模型`。
- `删除` 是彻底删除，删除前会询问 `彻底删除这张表情包？`。AI 的 `meme_delete` 只是停用。

### 在通讯平台上使用

表情包是 im 的一个插件，默认开启，可以在会话卡片的 `管理插件` 里按群、私聊开关。在关掉的地方，AI 没有这五个工具，也不会收到发表情包的提醒。见 [接入通讯平台](heads/im.md#插件)。

### 设置

| 键 | 界面上的名字 | 默认 | 说明 |
|---|---|---|---|
| `memes.reminder` | 自动提醒发表情包 | `true` | 关掉后就不掷骰子。 |
| `memes.reminder_probability` | 每次回复提醒的概率（%） | `3` | `0` 到 `100` |
| `memes.max_size_mb` | 单张上限（MiB） | `5` | `1` 到 `20` |

修改后当场生效，例如：

```sh
miyu config set memes.reminder_probability 10
```

`miyu memes` 这条命令是给核心启动包里的程序用的，你手动运行它只会输出 `usage: miyu-memes serve`。其他软件包通过总线上的名字 `memes`（`memes.add`、`memes.search`）往库里存图、在库里搜索，见 [总线](../../reference/bus.md)。

### 常见问题

| 看到的现象 | 解决方法 |
|---|---|
| `Not saved: no vision model could describe it (…)` | 配置 `models.vision`（见 [模型](models.md#三个用途)），或者让 AI 看过图以后自己写描述和用法。 |
| `Not saved: it is a screenshot, photo, document or chart, not a sticker.` | 看图的模型判断这不是表情包。如果确实要保存，在后台页自己上传。 |
| `msg works only in a chat.` | `msg` 只能在通讯平台的会话里使用，本机的会话使用 `path`。 |
| 后台页提示 `这张图缩不到能传的大小（动图不缩）` | 换一张小一点的图，或者调大 `memes.max_size_mb`。 |
| 后台页提示 `看图模型不支持图片，请在设置里换一个` | `models.vision` 配置的模型不能接收图片，换一个模型。 |
| 后台页提示 `未开启语义模型` | 安装 `embed`；也可能是 `models.embedding` 写成了 `off`。 |
