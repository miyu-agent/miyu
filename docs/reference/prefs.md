# 头自己的设置：`prefs.*`

界面包（清单是 `[ui]` 的那种，终端、网页这类头）自己的设置：皮肤、配色、主题、吉祥物这些，存在核心的数据根里，跟着人走，不跟着地址、设备走：用 `127.0.0.1`、`localhost`、局域网地址、手机打开同一个网页，看到的是同一份。核心不认里面的键，键和值由头自己定；只跟这台设备有关的（栏宽、侧边栏收没收、输入历史）留在头自己那里。英文版：[prefs.en.md](prefs.en.md)。

| 方法 | 做什么 |
|---|---|
| [`prefs.get`](#prefsget) | 读整份 |
| [`prefs.set`](#prefsset) | 改几个键，交回改好的整份 |

**谁能调**：只给人（头的连接）；扩展、平台桥调了回 `local_only`。要跟着别的头改的，订阅 `prefs` 流（带 `package`），之后推 `prefs.changed`（[推送](pushes.md)）。

**存在哪**：那个包的状态目录里的 `prefs.json`（`var/lib/<包编号>/prefs.json`，`MIYU_STATE_DIR` 指的就是它）。整份一个 JSON 对象，一行，Unix 上只有你自己读得到。这个名字归核心，头自己别在状态目录里用它；卸掉这个包，设置跟着状态目录一起删。

**上限**：整份写成 JSON（不带空白）最多 64 KiB（65536 字节）。

## `prefs.get`

**参数**：`{"package"}`，界面包的编号。**回应**：`{"values": {…}}`，整份；还没设过的是 `{}`。

## `prefs.set`

**参数**：`{"package", "values": {键: 值或 null}}`。照顶层的键并进去：写了的键整个换成新的值（值是什么 JSON 都行，对象也是整个换、不往里并），写 `null` 的删掉这个键，没写的不动。`values` 是 `{}` 的什么都不改。

**回应**：`{"values": {…}}`，并好的整份。改了的推给别的订着这个包的连接，你自己这个连接不推：回应里已经是整份了。

## 订阅

`subscribe {"stream": "prefs", "package"}`：回应 `{"values": {…}}`，这一刻的整份；之后别的连接改了推 `prefs.changed {"package", "values"}`，`values` 是改好的整份，照它整个换掉。一个连接一个包一个，再订阅换一个新的；`unsubscribe {"stream": "prefs", "package"}` 停掉。读得慢掉了队推 `resync {"stream": "prefs", "package"}`，重新订阅。

## 出错

| 原因码 | 什么时候 |
|---|---|
| `bad_params` | 没写 `package`；`values` 没写、不是对象；订阅没写 `package` |
| `unknown_package` | 没装这个包 |
| `not_a_ui_package` | 装着，但不是界面包 |
| `prefs_too_big` | 并好的整份超过 64 KiB，什么都没改；`data` 带 `size`（并好的有多少字节）、`limit`（65536） |
| `local_only` | 扩展、平台桥调的 |

```json
→ {"jsonrpc":"2.0","id":"1","method":"prefs.set","params":{"package":"miyu-webui","values":{"skin":"paper","mascot":null}}}
← {"jsonrpc":"2.0","id":"1","result":{"values":{"scheme":"dark","skin":"paper"}}}
```
