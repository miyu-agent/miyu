# 往界面上放东西

> <sub>本文档由 Anthropic 的 Claude Fable 5.2 生成。</sub>

如果你写了一个扩展，想让它出现在终端界面、网页界面这些“头”（连接核心的界面程序）上，例如侧边栏里的一块状态、首页上的一段内容、底栏的一小格、一整页可以修改的设置、一个自己的后台网页，或者托盘菜单里的一行，这一页讲每一种的做法。扩展本身怎么写见 [写扩展](extensions.md)。

## 几种放置位置

扩展可以往界面上放东西的位置有下面几种：

| 办法 | 出现的位置 | 清单 | 需要的能力 |
|---|---|---|---|
| 界面块：首页 | 终端：空会话首页上工作目录的下面；网页：新会话的首页 | `[[widget]]`，`place = "home"` | `widgets` |
| 界面块：侧边栏 | 终端：右边的侧边栏（窗口宽度在 120 列以上时才有）；网页：右边一栏里的 `面板` | `[[widget]]`，`place = "sidebar"` | `widgets` |
| 界面块：底栏 | 终端：输入框下面那一行；网页：输入框下面那一行的右边 | `[[widget]]`，`place = "status"` | `widgets` |
| 界面块：整页 | 终端：居中显示的一整页；网页：软件的独立整页。输入它的斜杠命令即可打开 | `[[widget]]`，`place = "page"` | `widgets` |
| 斜杠命令 | 两个头的命令菜单 | `[[slash]]` | `commands` |
| 后台页 | 网页设置页里的 `软件后台`；终端 `软件包` 页里的 `软件后台`；`miyu pkg page <编号>` | `[page]` | 如果自己启动网页服务、替页面申请登录票，需要 `login.page` |
| 托盘菜单一行 | 托盘菜单（macOS 上是菜单栏图标） | `[tray]`，并且必须有 `[page]` | 不需要另外申请 |

这几种都要求包带有常驻的程序：清单里要有 `[process]`（也就是扩展），能力写进 `[process] capabilities`。网页的配色和形状见 [写主题和外观](themes.md)。

## 界面块

界面块的做法是：在清单里声明有哪几块，由你的程序把内容推给核心，核心再转给正在看的头，由头自己画出来。

### 清单

在清单里声明界面块，例如：

```toml
[process]
args = ["serve"]
capabilities = ["widgets"]

[[widget]]
id = "now"
place = "sidebar"
title = { zh = "时间", en = "Time" }
```

| 字段 | 说明 |
|---|---|
| `id` | 在包里唯一：以小写字母开头，只能包含小写字母、数字和 `-`，最多 32 个字符。 |
| `place` | `home` 表示首页，`sidebar` 表示侧边栏，`status` 表示底栏，`page` 表示整页。 |
| `title` | 侧边栏和整页必须写，写法是 `{ zh = "…", en = "…" }`。 |
| `scope` | `global`（默认值）表示全局只有一份；`session` 表示每个会话一份，只能用于侧边栏。 |
| `command` | 只能用于整页：输入这个斜杠命令就会打开这一页，写法和 `[command] name` 相同。 |
| `state` | 写 `true` 时，内容来自包在会话里的状态（工具结果里的 `state`、`session.set_state`），程序没有在运行时也有内容；只能用于 `scope = "session"` 的块。 |

一个包最多 16 块。写错时会报 `bad_widget`，`miyu pkg install` 和 `miyu check` 会指出是哪一个字段写错了。

### 程序这一侧

你的程序按下面的方式处理界面块：

1. 有人开始看一块时，核心会发通知 `widget.watch {"widget", "session"?}`；最后一个人也不看了，核心会发 `widget.unwatch`。你的程序连接并完成握手以后，核心会为此刻有人在看的每一块补发一条 `widget.watch`。只在有人看的时候获取数据。
2. 推送内容：`widget.set {"widget", "session"?, "title"?, "content"}`。跟着会话的块必须写 `session`，全局的块不能写。`title` 会替换清单里的标题。核心回 `{"kept": true}`；如果是跟着会话的块，而这个会话没有人在看，核心会回 `{"kept": false}`，内容没有保存。
3. 撤掉内容：`widget.clear {"widget", "session"?}`。
4. 块上可以点击的东西会调用你的方法：先用 `package.methods {"methods": [{"name": "refresh"}]}` 登记方法，人点击以后核心会发 `method.call {"method", "params"}`，你回什么，头就拿到什么。

每推送一次，整块内容都会被替换。内容只保存在核心的内存里：核心或者你的程序重启以后，内容都从空的开始，等下一次 `widget.watch` 时再推送。你的程序停止以后，它的块会全部清空，头会显示 `<软件名> 没有运行`（网页显示 `未运行`）。同一块推给同一个头，最快 250 毫秒一次，中间的推送只保留最新的一次。

方法的每个字段和出错情况见 [扩展参考](../reference/extensions.md#widgetsetwidgetclearwidgetwatchwidgetunwatch)。

### 内容

界面块的内容是一个数组，一项是一段，每段用 `type` 说明是哪一种：

| `type` | 字段 |
|---|---|
| `heading` | `text` |
| `text` | `text`（可以换行，最多 2000 个字）、`tone` |
| `pairs` | `items`：`{"label", "value", "tone"?}`，最多 50 对 |
| `progress` | `value`（0 到 1，必须写）、`text`、`mark`（0 到 1，进度条上标出的位置） |
| `list` | `items`：`{"title", "detail"?, "right"?, "state"?, "action"?}`，最多 200 条；`right` 是一段文字，或者最多 4 段 `{"text", "tone"?}` |
| `buttons` | `items`：`{"label", "action"}`，最多 8 个 |
| `sections` | 只能用于整页：`items`（`{"id", "title", "subtitle"?, "badge"?, "state"?, "blocks"}`）、`add`（`{"label", "call", "options"?}`） |
| `form` | 只能用于整页：`id`、`fields`（`{"key", "label", "hint"?, "control", "value", …}`）、`submit`（`{"label", "call", "params"?}`） |

例如：

```json
[
  {"type": "heading", "text": "本周"},
  {"type": "text", "text": {"zh": "晴 23°C", "en": "Sunny 23°C"}, "tone": "accent"},
  {"type": "pairs", "items": [{"label": "湿度", "value": "40%"}]},
  {"type": "progress", "value": 0.42, "text": "索引 42%"},
  {"type": "list", "items": [{"title": "构建 #128", "right": "3m", "state": "doing", "action": {"call": "build.open", "params": {"id": 128}}}]},
  {"type": "buttons", "items": [{"label": "刷新", "action": {"call": "refresh"}}, {"label": "详情", "action": {"open": "detail"}}]}
]
```

- 文字可以是字符串，也可以是语言到文字的对应表（`{"zh": "…", "en": "…"}`，认 `zh`、`en`、`ja`），核心会按照头的语言挑选。`text` 段的文字最多 2000 个字，其他的最多 200 个字。内容只画成纯文字：不认 Markdown 和 HTML，不允许有控制字符（`text` 段里的换行除外）。
- `tone` 可以是 `normal`、`dim`、`accent`、`good`、`warn`、`bad`，具体颜色由头按照自己的配色决定。列表的 `state` 可以是 `todo`、`doing`、`done`、`failed`、`off`；分区的 `state` 可以是 `good`、`warn`、`bad`、`off`。
- `action` 有三种写法：`{"call": <你登记的方法>, "params"?}`；`{"open": <你的一个整页块的 id>}`；`{"copy": <字>}`，把字复制到剪贴板。
- 整块最多 64 段，写成 JSON 最多 64 KiB；`home` 最多 8 段；`status` 只能是一段一行的 `text`，最多 40 列（中文算两列）。

**整页的表单**：`form` 的 `control` 可以是 `toggle`、`select`（带 `options: [{"value", "label"}]`）、`text`、`textarea`、`list`、`number`（可以带 `min`、`max`、`integer`）。`value` 写 `null` 表示使用默认值，`default` 写成 `{"text", "value"?}`，告诉头默认值是什么。人点击保存以后，头会通过 `package.call` 调用你的 `submit.call`，参数是 `submit.params` 加上 `{"values": {只有改过的格}}`。你可以回 `{"saved": true}`；或者回 `{"saved": false, "problems": [{"key", "message"}]}`（问题显示在对应的字段下面）；或者回 `{"saved": false, "message"}`（显示在表单底下）。保存好以后，照常用 `widget.set` 推送新的内容。`sections` 的 `add` 被点击时，会调用 `add.call`，参数是 `{"value": <选的那一项>}`，你可以回 `{"id": <新的那一项>}`。

### 两个头的画法

两个头画界面块的方式和上限不同：

| | 终端 | 网页 |
|---|---|---|
| 首页 | 最多 3 块，每块最多 6 行，合起来最多 12 行 | 最多 3 块，每块最多 8 行 |
| 侧边栏 | 每块最多 10 行，排在侧边栏自带的几段后面 | `面板` 里最多 8 块，每块最多 12 行 |
| 底栏 | 最多 3 格，每格最多 40 列 | 最多 3 格 |
| 整页 | 居中显示，可以滚动查看 | 软件的独立整页 |
| `sections`、`form` | 不画 | 会画出来，可以修改、保存 |

终端里放不下时：列表会像待办那样收起，`text` 会截断并加上 `…`，其他类型的段如果整段放不下，就连同后面的段一起不画；点击块里的其他地方可以展开，再点一次收起。终端侧边栏里各段的先后顺序可以在配置文件里调整：`tui.side` 一项是一段，你的块写成 `pkg:<包的编号>/<块的 id>`。

### 整页和斜杠命令

写了 `command` 的整页块，它的命令会列进 `command.catalog`（带有 `opens`）。人输入这个命令以后，`command.run` 会回 `{"command", "opens": {"package", "widget"}, "said": ""}`，头就打开那一页；这个命令不会进入会话日志。通讯平台上的群聊、私聊里不列出、也不识别这种命令。

如果斜杠命令只需要回一句话，就写 `[[slash]]`：人输入命令以后，核心调用你的 `command.call`，你回 `{"said": "<回执>"}` 即可，详细的见 [写扩展](extensions.md)。

## 后台页

后台页是给你的包的一个网页，可以用来做设置、管理和查看数据。清单里二选一：

- `[page] dir = "page"`：使用静态文件，目录里必须有 `index.html`。
- `[page] server = true`：由你的程序自己启动网页服务。

### 静态的后台页

静态的后台页可以从这几个地方打开：网页设置页里的 `软件后台`，终端 `软件包` 页里的 `软件后台`（按 `Enter` 在浏览器里打开），以及命令行 `miyu pkg page <编号>`（加 `--print` 只输出网址）。

- 没有安装网页界面也能打开：`miyu pkg page` 会让核心在 `127.0.0.1` 上启动一个后台页服务，十分钟内没有请求、也没有打开着的推送流时就会关闭。
- 页面放在一个隔离的框里，通过一条消息通道和外面通信。框载入完成以后，外面会用 `postMessage({"miyu": "port"}, "*", [port])` 交进来一个 `MessagePort`，而且只交一次，所以页面要做成单页（在框里跳到其他页面，通道就没有了）。
- 通道上使用 JSON-RPC 2.0，你能调用的只有下面四个方法：

  | 方法 | 参数 | 回应 |
  |---|---|---|
  | `context` | 无 | `{"package", "language", "theme", "colors"?}`：分别是你的包、语言、深浅（`light`、`dark`）；在网页里还会另外带几个主色 |
  | `settings.get` | 无 | `{"items", "values"}`：你这个包的配置项和它们的最终值 |
  | `settings.set` | `{"changes": [...]}`，写法和 `config.set` 相同 | 只能写以 `<你的前缀>.` 开头的键；如果有其他键，整个请求都不执行，回 `forbidden` |
  | `call` | `{"method", "params"?}` | 转给你的程序（`method.call`），你回什么，页面就拿到什么 |

  外面还会推送这几种消息：`settings.changed`（你的配置项变了）、`theme.changed`（深浅换了）、`package.event`（你的程序通过 `package.emit` 推给页面的消息）、`resync`（推送掉队了，需要重新拉取一遍）。
- 可以照着抄的例子：表情包 `memes` 包里的 `package/page/`（通道的写法在 `app.js` 里）。

### 自己启动服务的后台页

- 只有扩展能写这种后台页。用 `package.methods` 登记一个 `status` 方法，在回应里写上 `web`，值是服务实际监听的端口（还没有开始监听时不写）。
- 头和 `miyu pkg page` 会根据它打开 `http://127.0.0.1:<端口>/#miyu=<…>`：`#` 后面是一段用 base64url（不带 `=`）编码的 JSON `{"v": 1, "ticket", "lang"}`，其中 `ticket` 是一张有效期 60 秒、只能用一次的登录票。你的服务要拿它换设备令牌：在扩展的连接上调用 `login.redeem {"ticket"}`，核心回 `{"login": {"token", "expires"}}`，之后用这个设备令牌（握手凭据 `login`）替页面连接核心。
- 如果你要自己申请登录票（例如页面过期以后重新打开）：声明能力 `login.page`，在扩展连接上调用 `login.ticket {"via": "page"}`。这种票只能在本机的连接上兑换；只能为核实过是从本机来的请求申请和兑换登录票，从别处来的页面一律使用访问密码。要遵守的几条规则见 [扩展参考](../reference/extensions.md) 的「替本机页面换登录票」。
- 例子：接入通讯平台的 `im` 包（清单是 `im/package.toml`）。

## 托盘菜单里的一行

清单的写法如下：

```toml
[page]
dir = "page"

[tray]
name = { zh = "打开时钟后台", en = "Open Clock" }
```

这样托盘菜单里会多出一行，点击后用浏览器打开你的后台页（和 `miyu pkg page <编号>` 一样）。一个包只能有一行，只能打开自己的后台页。没有写 `[page]`，或者写了 `name` 以外的字段，会报 `bad_tray`。包要处于安装并开启的状态，托盘才会列出它。

## 网页里的前端包

出厂的网页界面是由一个个网页包拼起来的（每个网页包是一个 `manifest.json` 加一个 `index.js`）。第三方能带进网页的只有外观包的 `[theme.web]`，对外说明的用途是配色方案、主题和吉祥物（见 [写主题和外观](themes.md)）。如果想往网页里加其他功能（新的面板、按钮、页面），目前没有对第三方开放的写法，要用上面的界面块和后台页。

## 例子：侧边栏里的时钟

下面写一个扩展，在侧边栏放一块 `时间`，显示现在的时刻和日期，每分钟更新一次。它用 Python 开发包 `miyu_ext` 写（见 [写扩展](extensions.md#用开发包写)）。

1. 建立目录，把开发包里的 `miyu_ext.py` 复制到 `bin/` 里：

   ```text
   clock/
   ├── package.toml
   └── bin/
       ├── miyu-clock      # 下面的程序，Linux、macOS 上带执行权限
       └── miyu_ext.py
   ```

2. 写 `package.toml`：

   ```toml
   [package]
   version = "0.1.0"
   protocol = [1, 1]
   name = { zh = "时钟", en = "Clock" }
   summary = { zh = "侧边栏显示现在几点", en = "Shows the time in the sidebar" }

   [command]
   name = "clock"
   program = "miyu-clock"
   about = { zh = "时钟扩展", en = "Clock extension" }

   [process]
   args = ["serve"]
   capabilities = ["widgets"]

   [[widget]]
   id = "now"
   place = "sidebar"
   title = { zh = "时间", en = "Time" }
   ```

3. 写 `bin/miyu-clock`：

   ```python
   #!/usr/bin/env python3
   """侧边栏里的时钟：每分钟推一次现在几点。"""
   import os
   import sys
   import threading
   import time

   sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

   from miyu_ext import Extension  # noqa: E402

   ext = Extension("clock", "0.1.0")
   wake = threading.Event()

   @ext.on("widget.watch")
   def watched(params):
       wake.set()                     # 有人开始看了：马上推一次

   @ext.ready
   def tick():
       while True:
           now = time.localtime()
           ext.call("widget.set", {"widget": "now", "content": [
               {"type": "text", "text": time.strftime("%H:%M", now), "tone": "accent"},
               {"type": "pairs", "items": [
                   {"label": {"zh": "日期", "en": "Date"}, "value": time.strftime("%Y-%m-%d", now)},
               ]},
           ]})
           wake.wait(60 - now.tm_sec)     # 到下一分钟，或者有人开始看
           wake.clear()

   if __name__ == "__main__":
       if sys.argv[1:] == ["serve"]:
           ext.run()
       else:
           print("usage: miyu-clock serve (started by the Miyu core)", file=sys.stderr)
           sys.exit(2)
   ```

   在 Windows 上这份文件同样可以用：核心找不到 `bin/miyu-clock.exe` 时，会根据第一行的 `#!` 用 `PATH` 里的 `python` 启动它，不需要执行权限。文件名按照清单的 `program` 写成 `miyu-clock`，不加 `.py`；需要先安装好 Python 3，并且能在 `PATH` 里找到 `python`。详细的见 [写一个 Miyu 软件包：三个平台](packages.md#三个平台)。

4. 安装：`miyu pkg install ./clock`。安装时屏幕上会列出它要的能力 `widgets`，输入 `Y`；询问 `现在启用 'clock'？[Y/n]` 时也回答是。
5. 查看效果：
   - 终端：把窗口拉宽到 120 列以上，右边的侧边栏里会多出一段 `时间`，下面一行是强调色的 `14:05`，再下面一行是 `日期 2026-11-14`。
   - 网页：点开右边一栏的 `面板`，里面有一块 `时间`，内容和终端里相同。

如果没有出现：先在 `软件包` 页看它是不是在运行，再看数据根（默认是 `~/.miyu`，Windows 上是 `%USERPROFILE%\.miyu`）里的 `var/log/clock/stderr`。

## 排查问题

常见的问题和处理方法如下：

| 看到的现象 | 处理方法 |
|---|---|
| `widget.set` 回 `bad_widget_content` | `data.path` 指出错在哪里（例如 `content[1].items[0].label`），`data.problem` 说明是哪一种错误：`type`、`missing`、`too_long`、`too_many`、`too_big`、`control_char`、`tone`、`state`、`value`、`action`、`place`、`unknown`、`duplicate` |
| `widget.set` 回 `unknown_widget` | `widget` 不是清单里写的某个 `id` |
| `widget.set` 回 `bad_params` | 跟着会话的块没有写 `session`，或者全局的块写了 `session` |
| 调用方法时回 `needs_approval` | 清单的 `capabilities` 里没有写 `widgets`（或者其他那一项能力），加上以后重新安装一次 |
| 块上显示 `没有运行` | 程序没有在运行：到 `软件包` 页查看原因和标准错误的最后几行 |
| 托盘里没有那一行 | 包没有开启，或者后台页的 `index.html` 不存在 |
