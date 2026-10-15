## Writing a Miyu package: a guide for third-party authors

Status: the English edition of `README.md` (2026-10-13). The Chinese page is the source; the two are changed together. This page describes the core as it is now. The blueprints have the full details; this page picks out what an author needs to know.

### 1. What to know first

1. **One folder is one package**, and the folder name is its ID. The manifest is `package.toml` in that folder; everything else sits next to it.
2. **What a package can carry**: one program (one of three kinds: an **extension** the core starts and keeps running; a **worker** the core starts on demand; an **interface** a person opens), ready-made **MCP servers** (section 3, item 8; no program needed), **skills** and **scripts** (section 3, item 9; no program needed), a **mascot** for the terminal interface, and an **admin page** (a settings page opened in the web interface). The most common is an extension: it gives the AI new tools or connects a chat platform.
3. **An extension is a separate program**, written in any language. The core starts it and talks JSON-RPC 2.0 over standard input and output, one message per line. The extension says hello first, then registers its tools; when the AI calls one of them, the core asks the extension to do it.
4. **Install and remove work like pacman**: `miyu pkg install ./my-package`, `miyu pkg remove <id>`. Installing copies the whole folder into the administrator's home (`~/.miyu/home/admin/packages/<id>/`) and takes effect at once, with no core restart. Removing cleans up everything: the package folder, the package's own state directory, and its settings.
5. **What your program may do is declared in the manifest and approved by the administrator**: list the capabilities you need in the manifest; the administrator approves them when turning the package on (section 6).

### 2. What a package looks like

```text
hello/
├── package.toml          # the manifest
├── bin/
│   └── miyu-hello        # the program; miyu-hello.exe on Windows
└── page/
    └── index.html        # the admin page, optional
```

1. **The ID** (the folder name): starts with a lowercase letter, uses only lowercase letters, digits, `-` and `_`, at most 64 characters. It must not clash with the packages shipped with Miyu (`basesystem`, `memory`, `roleplay`, `mermaid`, `net`, `onebot`, `tui`, `web` and so on).
2. **The program lives under `bin/`**, named as `[command] program` says, with `.exe` added on Windows. The core looks in the package's `bin/` first, then next to the `miyu` executable, and **never in `PATH`**: no other program can pose as yours. On Linux and macOS the program needs the executable bit; a script needs its `#!` line.
3. **Do not write into the package folder after installing**: every file's hash is recorded at install time, and `miyu pkg check` reports files you add later as "extra" and files you change as "modified". Keep runtime state in the working directory the core gives you (section 4, item 2).

### 3. The manifest

A complete manifest for an extension:

```toml
[package]
version = "0.1.0"                                   # optional, shown to people only
protocol = [1, 1]                                   # the protocol major versions you speak [lowest, highest]; only 1 for now
name = { en = "Hello", zh = "打招呼" }
summary = { en = "Greets people by name", zh = "照名字打招呼" }   # optional
icon = "hand"                                       # a Lucide icon name, optional

[command]                                           # required for an extension
name = "hello"                                      # `miyu hello …` is handed to your program
program = "miyu-hello"                              # the program name under bin/, without a path
about = { en = "Hello extension", zh = "打招呼扩展" }

[process]                                           # having this makes it an extension
args = ["serve"]                                    # arguments the core starts it with
start = "manual"                                    # manual: started when switched on; always: started with the core
capabilities = ["tools"]                            # what it needs, see section 6

[settings.greeting]                                 # a setting; its key is hello.greeting
type = "text"
max = 40
default = "Hello"
layers = ["system"]
applies = "now"
name = { en = "Greeting", zh = "问候语" }
```

1. **Text for people** (`name`, `summary`, `about`, a setting's `name` and `description`) is written as "language to sentence". Only `zh`, `en` and `ja` are accepted; interfaces pick the person's language and fall back to `en`, `zh`, `ja`.
2. **`[command]`**: `name` starts with a lowercase letter, uses only lowercase letters, digits and `-`, at most 32 characters. If it clashes with a built-in subcommand (`ask`, `config`, `pkg`, `web` and so on), the built-in wins and yours is not handed over. When someone types `miyu hello args…`, the core passes the arguments to your program unchanged and exits with your exit code; `miyu help hello` becomes `--help`. So besides `serve`, your program should understand what people type.
3. **Settings** `[settings.<name>]`: `type` is one of `bool`, `int` (`min`, `max`), `option` (`choices`), `text` (`max`, default 200), `name`, `url`, `secret`, `list` (`element`); `layers` is one or both of `system` and `personal`; `applies` is one of `now`, `new_session`, `next_turn`, `program_start`. Your settings get their own group on the "Packages" page of the settings screen. **Do not read configuration files yourself**: the core hands you the values at hello and pushes changes to you (section 4).
4. **Features** `[features.<id>]`: one switch in a preset turns a group of tools on or off. Without this table the whole package is one feature; once you write some, every tool you register must be listed under one of them, or it will not be registered.

   ```toml
   [features.greet]
   name = { en = "Greeting", zh = "打招呼" }
   tools = ["hello_greet"]
   ```

5. **The package's own check** `[check] args = ["check"]`: `miyu check` runs `<program> check`; print one JSON object per line on standard output for each problem (`kind`, `file`, `level`, `message` are required; `line`, `column`, `code`, `key`, `rule`, `source` are optional). Exit 0 when there are only warnings, 1 when there are errors. It may run for at most 30 seconds.
6. **The admin page** `[page] dir = "page"`: the directory must contain `index.html`. The page calls the methods you register through a channel the web interface provides (section 4, item 7).
7. A manifest that cannot be read is not installed: `miyu pkg install` says which line is wrong; `miyu check` checks it too.
   A package whose names clash with an installed package is not installed either (like pacman's conflicts): the subcommand, feature ids, tool names (listed under features, and scripts), skills, MCP servers, knowledge base ids and slash command templates (which also cannot reuse a name of Miyu's built-in commands). `miyu pkg install` prints one line per clash, naming the package that already has it. Prefixing names with your package id is a good idea.
8. **Carrying MCP servers** `[[mcp]]`: one table per server. The core starts the server process from it, speaks MCP over standard input and output, and registers the server's tools. A package that carries only servers and no program does not write `protocol`, `[command]` or `[process]`; an extension can carry servers too, and they start and stop with the extension.

   ```toml
   [package]
   name = { en = "Reference MCP", zh = "参考 MCP 服务" }

   [[mcp]]
   name = "files"                                    # server name: lowercase letters, digits, -, 1 to 32 characters
   command = ["npx", "-y", "@modelcontextprotocol/server-filesystem", "/srv/notes"]
   env = { TOKEN = { secret = "notes" } }            # optional; values can refer to a secret { secret = … } or an environment variable { env = … }
   venues = ["local"]                                # optional; when left out, only local sessions get it
   timeout_ms = 600000                               # optional: how long one call may take, 1000 to 3600000
   ```

   - **The tools the AI sees** are named `mcp__<server>__<tool>`; descriptions and parameters are passed on as the server gives them (descriptions longer than 2048 characters are cut). In a preset each server is one feature, `mcp-<server>`, which can be turned off whole or tool by tool.
   - **When people are asked**: a tool the server annotates with `readOnlyHint: true` is never asked about; everything else counts as network access, and at the workspace level a person must confirm it (`miyu ask`, which cannot ask, just does not do it). Please mark your server's read-only tools.
   - **How it starts**: the first word of the command is looked up in the package's `bin/`, then next to `miyu`, and finally in `PATH` (`npx`, `uvx`, `docker` and the like). Miyu does not install Node or Python for people: if your server runs through `npx` or `uvx`, say clearly what must be installed first. The working directory is `~/.miyu/state/mcp/<server>/`, outside the sandbox; standard error goes to `~/.miyu/state/logs/mcp-<server>.stderr`.
   - **Switch and status**: a package that carries only servers is off after installing and is turned on from the "Packages" page; the page shows one row per server, telling whether it is running (with the process ID), waiting to restart, or stopped (the reason and the last lines of standard error), and a stopped one can be restarted there. A server that keeps failing to start is retried with backoff and stops after 5 tries.
   - **Which MCP version**: both eras are understood (version 2026-07-28 dropped `initialize`; earlier versions say hello first), so your server may speak either. Only tools are used; resources, prompts, `sampling` and the rest are not.
   - **Without making a package**: write the server into the administrator's personal settings, `~/.miyu/home/admin/settings.toml`, one `[mcp.<server>]` table per server, with the same keys as above minus `name`. Changes take effect at once, and when a package carries a server of the same name, the one in the settings wins (construction step F-9).
9. **Carrying skills and scripts** (`docs/blueprint/skills.md`): put them under `skills/` and `scripts/` in the package folder; the manifest does not list them. A package that carries only these has a manifest with just `[package]` (no `protocol`) and no switch; each skill and script is turned on or off in presets on its own.

   ```text
   notes/
   ├── package.toml          # only [package]
   ├── skills/
   │   └── take-notes/
   │       └── SKILL.md
   └── scripts/
       └── dice.py
   ```

   - **Skills** follow the public Agent Skills format: `skills/<name>/SKILL.md`, whose YAML front matter has `name` (lowercase letters, digits, `-`, the same as the folder name) and `description` (for the model, in English: what it is and when to use it). The name and one-line summary shown to people go in `metadata` as `display_name` and `summary` (English), with `display_name_zh`, `summary_zh`, `display_name_ja`, `summary_ja` for Chinese and Japanese. Put `description` in double quotes: a description containing "colon followed by a space" cannot be read otherwise. The AI normally sees only the name and description and reads the body when it needs it; it can also read, but not change, the other files in the folder (references, bundled scripts).
   - **Scripts**: one file is one tool, named after the file (without the extension, with characters outside `[A-Za-z0-9_-]` replaced by `_`). The leading comment carries the description and parameters:

     ```python
     #!/usr/bin/env python3
     # Display name: Dice
     # 显示名称：掷骰子
     # Description: Roll dice and return each face and the total.
     # 描述：掷骰子，交回每一颗和总数。
     # Timeout: 10
     # Parameters:
     # {"type": "object", "properties": {"count": {"type": "integer"}, "sides": {"type": "integer"}}, "required": ["count", "sides"]}
     ```

     The parameters arrive as JSON, once in the environment variable `MIYU_ARGS_JSON` and once on standard input; standard output is the result handed to the AI, and a non-zero exit code counts as failure; a writable cache directory is in `MIYU_SCRIPT_CACHE_DIR`. The script is started through its `#!` line (on Windows the interpreter is chosen by extension: `.py` uses `python`, `.ps1` PowerShell, `.js` `node`). It runs under the same rules as the AI running a command: inside the sandbox (writes only in the workspace, Miyu's data directory unreadable, the script file itself readable), and when the sandbox is unavailable a person must confirm. Keep the description and parameters short: they go out with every request.
   - Badly written ones are not registered; `miyu check` says which one and why, and also reports names that clash with other tools.
   - **Slash command templates** follow Claude Code: `commands/<name>.md`. When someone types `/<name> arguments`, the body is sent to the AI as that person's message, with `$ARGUMENTS` replaced by the arguments (`$ARGUMENTS[0]` is the first one); if the body has no placeholder, the arguments are appended at the end. Optional YAML front matter: `description` is the line shown in the command menu (English), `argument-hint` says what follows the name, and Chinese and Japanese descriptions go in `metadata` as `summary_zh` and `summary_ja`. Names follow the subcommand rule; a name Miyu already uses for a built-in command is ignored (the built-in wins). The timeline shows the line that was typed (`docs/blueprint/slash.md`).
   - **Slash commands run by your program** (extensions): one `[[slash]]` table per command with `name`, `summary` (language to sentence) and optionally `argument` (a hint for what follows the name); `[process] capabilities` must include `commands`. When someone types it, the core sends your program `command.call {command, argument, text, session, by, owner, language}`; answer `{"said": "…"}` (several lines are fine) and that text is the receipt the person sees. A JSON-RPC error shows "Command failed" with your message. Your program has to `provide` once after it starts (with an empty `tools` list if it has none) so the core knows your connection. The same people as for Miyu's built-in commands may use it (the owner, and group owners and managers); check `by` yourself if you need more.
   - If you do not want a package, put them straight into `~/.miyu/home/admin/skills/`, `scripts/` and `commands/`; they work the same (on a name clash these win) and are not managed as packages.

### 4. Writing an extension program

1. **How it is started**: the program is the one under `bin/`, with `[process] args` as its arguments. The environment is the core's, plus `MIYU_HOME` (the data root) and `MIYU_RESOURCES` (the resource directory). The working directory is the package's state directory, `<data root>/state/packages/<id>/`: keep your database and caches there; they are removed with the package. Standard error goes to `<data root>/state/logs/<id>.stderr`; look there first when something goes wrong.
2. **One message per line**: each message is one line of compact JSON ending in `\n`, at most 1 MiB. The `id` of a request you send must be a string (1 to 128 bytes).
3. **Hello**: the first message is `hello`:

   ```json
   {"jsonrpc":"2.0","id":"h1","method":"hello","params":{"protocol":[1,1],"head":{"kind":"hello","version":"0.1.0"}}}
   ```

   No credentials are needed: the core handed you your standard input and output itself. In the reply, `config` holds the final values of your package's settings (`{"hello.greeting":"Hello"}`; secrets are the real values; unset keys are left out), and `language` says which language text for people should use. When settings change later, the core pushes `extension.config {"keys": {"hello.greeting": "Hi"}}`, with only the keys that changed, and `null` for one that became unset. A connection that has not said hello within 10 seconds is dropped.
4. **Registering tools**: after hello, send `provide`; `tools` is the full set of tools you offer right now, and each `provide` replaces the whole set:

   ```json
   {"jsonrpc":"2.0","id":"p1","method":"provide","params":{"tools":[{"name":"hello_greet","description":"Greet someone by name.","input_schema":{"type":"object","properties":{"name":{"type":"string"}},"required":["name"]},"access":"read","venues":["local"]}]}}
   ```

   - `name`: English letters, digits, `_`, `-`, 1 to 64 characters, not clashing with the core's tools or other packages'. Prefixing it with your package ID is a good idea.
   - `description` and the descriptions in `input_schema` are for the model: write short English sentences, at most three per tool (what it is, when to use it, the one pitfall people hit most). Knowledge that only matters after the call belongs in the tool's output, not in the description. This text goes out with every request; the shorter, the cheaper.
   - `access`: what the tool touches. `read` (only looks; touches nothing on this machine and sends nothing out of Miyu), `write` (changes files), `execute` (runs commands), `network` (goes online), `outbound` (sends something out of Miyu), `venue` (acts on a chat platform: recalling, muting and the like). The permission policy decides from it whether to ask a person, so **tell the truth**: marking a tool that sends things out as `read` deceives the administrator.
   - `venues`: which kinds of sessions get it, one or more of `local` (on this machine), `private` (platform private chats), `group` (groups).
   - `timeout_ms`: optional, 1000 to 600000, default 60000.
   - `display_name`: optional, the name people see, per language: `{"zh": "打招呼", "en": "Greet"}`. The timeline shows it in the reader's language and falls back to `name`. It is for people, not for the model.
   - A wrong entry rejects the whole set; the reply's `data` says which tool and what is wrong (`{"tool":"hello_greet","problem":"name"}`).
5. **Being called**: when the AI calls one of your tools, the core sends you `tool.call`:

   ```json
   {"jsonrpc":"2.0","id":"core-1","method":"tool.call","params":{"session":"…","call_id":"…","tool":"hello_greet","args":{"name":"Ann"},"cwd":"/home/me/src","owner":true}}
   ```

   `args` has already been corrected against your `input_schema`. `by` is who asked her to do it (left out when nobody did), and `owner` is whether the core recognizes that person as the owner: for tools that act in groups, deciding who may use them is up to you. Reply:

   ```json
   {"jsonrpc":"2.0","id":"core-1","result":{"blocks":[{"type":"text","text":"Hello, Ann!"}],"error":false}}
   ```

   `blocks` are content blocks (text `{"type":"text","text":…}`; for an image, upload it with `blob.put` first, then send `{"type":"image","blob":…,"media_type":…,"width":…,"height":…}`). On failure, still reply with a result, set `error` to `true`, and explain the reason for the model; a JSON-RPC error also counts as a failed call. Several `tool.call`s may arrive at once; answer each by its `id`.
6. **Timeouts and stopping**: when `timeout_ms` passes without an answer, or a person interrupts, the core sends the notification `tool.cancel {"session","call_id"}` (no `id`, no reply needed). Drop that call when you get it; what has already been done cannot be undone, so say so truthfully if you still answer.
7. **Methods for the admin page** (only if you have one): register them with `package.methods {"methods":[{"name":"status"}]}`; when the page calls one, the core sends `method.call {"method","params"}`, and whatever you reply is what the page receives. Answer an unknown method with JSON-RPC `-32601`.
8. **Exiting**: reaching the end of standard input means "please exit", on all three platforms; a program that has not exited 5 seconds later is killed.
9. **When it crashes**: exiting on your own (even with exit code 0), failing the hello, or closing standard output each count as one failure. The core starts you again after 1, 2, 4, 8, 16 seconds (at most 60) and stops after 5 failures in a row. **Exit code 1 is for problems a restart cannot fix**, such as bad configuration or a port in use: the core stops at once without retrying until a person fixes it and turns it on again. After 60 seconds of running, the failure count starts over.
10. **What not to do**: do not read or write elsewhere in the data root (sessions, other packages, configuration files, the secrets file); ask for information through the protocol. Methods meant only for people on this machine, such as `extension.*` and `package.install`, are refused to extensions (`local_only`).

### 5. A minimal extension

`bin/miyu-hello` (Python; `chmod +x` on Linux and macOS):

```python
#!/usr/bin/env python3
import json
import sys

TOOLS = [{
    "name": "hello_greet",
    "description": "Greet someone by name.",
    "input_schema": {"type": "object", "properties": {"name": {"type": "string"}}, "required": ["name"]},
    "access": "read",
    "venues": ["local"],
}]


def send(message):
    sys.stdout.write(json.dumps(message, ensure_ascii=False) + "\n")
    sys.stdout.flush()


def serve():
    send({"jsonrpc": "2.0", "id": "h1", "method": "hello",
          "params": {"protocol": [1, 1], "head": {"kind": "hello", "version": "0.1.0"}}})
    greeting = "Hello"
    for line in sys.stdin:  # the end of input means the core asks you to exit
        message = json.loads(line)
        if message.get("id") == "h1":
            if "error" in message:
                sys.exit(1)  # hello refused: restarting will not help
            greeting = message["result"].get("config", {}).get("hello.greeting", greeting)
            send({"jsonrpc": "2.0", "id": "p1", "method": "provide", "params": {"tools": TOOLS}})
        elif message.get("method") == "extension.config":
            greeting = message["params"]["keys"].get("hello.greeting") or "Hello"
        elif message.get("method") == "tool.call":
            name = message["params"]["args"].get("name", "there")
            send({"jsonrpc": "2.0", "id": message["id"],
                  "result": {"blocks": [{"type": "text", "text": f"{greeting}, {name}!"}]}})


if __name__ == "__main__":
    if sys.argv[1:] == ["serve"]:
        serve()
    else:
        print("usage: miyu hello serve (started by the Miyu core)", file=sys.stderr)
        sys.exit(2)
```

On Windows, `bin/` needs an `.exe`: package it with PyInstaller, or use a language that compiles to an executable.

### 6. Capabilities and approval

1. List what you need in `[process] capabilities`. Only these twelve are accepted: `tools` (register tools in the tool catalog), `commands` (provide slash commands), `context.inject` (add content to the context when a turn starts), `tool.guard` (allow, refuse, or ask a person before a tool runs), `tool.rewrite` (rewrite a tool's result after it runs), `events.read` (read a session's raw events), `events.write` (append events under your own name), `sessions.drive` (send messages, create sub-sessions, and other ways of driving sessions), `act_for_external` (issue commands on behalf of people on a chat platform; for platform bridges only), `network` (go online), `fs.read` and `fs.write` (read and write given paths). For now the parts of the protocol a third party can use are registering tools (`provide`) and admin page methods; commands and hooks are not open yet.
2. The administrator approves them when turning your extension on: everything you declare at that moment, in one go. After an upgrade that asks for more, the next time it is turned on only the new ones are asked about. An extension with unapproved capabilities is not started.
3. Blocking methods by approved capabilities and running inside a sandbox will come with the multi-user and sandbox work; neither is enforced yet. **Declare honestly**: undeclared use will be blocked later.

### 7. Install, try, inspect

1. Install: `miyu pkg install ./hello`. It first prints what it will do (which program, which commands, which capabilities, how big) and asks `[Y/n]`; `--yes` skips the question. Upgrading is installing again: the running copy is stopped, the files are replaced, and it is started again according to its switch.
2. Turn it on: after installing, it asks "Turn it on now? [Y/n]"; answering yes approves the capabilities listed above and turns it on in one step (`--yes` counts as yes). If you say no, turn it on and approve it later on the "Packages" page of the terminal or web interface (`manual` ones start once turned on; `always` ones start once approved).
3. Look: `miyu pkg info hello` (name, version, size, install time), `miyu pkg files hello` (which files were installed), `miyu pkg check hello` (whether installed files changed). The "Packages" page shows whether it is running, waiting to restart, or stopped (with the reason and the last 20 lines of standard error).
4. Logs: your standard error is in `~/.miyu/state/logs/hello.stderr` (moved to `.old` before a start once it passes 1 MiB); the core's own log is `~/.miyu/state/logs/core.log`, with one line each for starting, hello, exiting, backing off and stopping.
5. Changing things during development: after a change, `miyu pkg install ./hello` once more swaps it in. You can also try it in a separate data root (`MIYU_HOME=/tmp/miyu-try miyu …`) without touching the one you use every day.
6. Remove: `miyu pkg remove hello`. Its settings and `state/packages/hello/` are removed with it; entries in the secrets file are kept (something else may use them).

### 8. Three platforms

1. The program name is as the manifest says; on Windows the core looks for `<name>.exe`.
2. On Linux and macOS the program needs the executable bit; installing copies files as they are, permissions included.
3. macOS (Apple silicon only): programs downloaded from the internet carry the quarantine flag, and Gatekeeper blocks unsigned, unnotarized ones. Please sign and notarize programs you publish; programs you build and install yourself are not affected.
4. "Please exit" is the end of standard input on all three platforms; do not rely on signals.

### 9. Decisions (made by the project owner on 2026-10-11, following the recommendations)

1. **Turning extensions on and approving capabilities from the command line**: after installing, `miyu pkg install` asks "Turn it on now?", and answering yes approves the capabilities listed in the printout and turns it on (done in construction step F-8 4th patch; section 7, item 2 follows it).
2. **The language of this page**: finalize the Chinese first, and publish an English edition before 0.0.1 goes out (this page).
3. **`author`, `license`, `url` in the manifest**: added together with the package repository work.
4. **How skills, scripts and MCP servers go into packages**: settled in `skills.md` and `mcp.md` and added to this page during the construction steps from X-2 and X-10 on. MCP servers were added to section 3, item 8 in step X-5; skills and scripts to section 3, item 9 in step X-13 (middle).
