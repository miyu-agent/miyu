# Scripts, skills, command templates

None of these need a long-running program; one file is enough. Chinese version: [scripts-skills.md](scripts-skills.md).

| | What it is | How the AI uses it |
|---|---|---|
| **Script** | An executable file; one tool | Called like any other tool |
| **Skill** | A `SKILL.md`; a way of doing something | Normally sees only its name and one line; reads the body when it needs it |
| **Command template** | A `<name>.md`; a slash command | When a person types `/<name>`, the body is sent to the AI as that person's message |

Where: in a package, under the package folder's `scripts/`, `skills/`, `commands/`, with a manifest of just `[package]`; just for you, straight into `~/.miyu/scripts/`, `skills/`, `commands/` (same names there win). The next session sees them; each script, skill and template is switched separately in presets. Broken ones, and scripts with the same name as another tool, aren't registered, and `miyu check` says which and why; if it says the core hasn't picked a new one up yet, check again shortly.

Links work too, pointing at files or folders elsewhere (say, kept together in a repository of your own): what the link points at counts, and the AI sees the real path; broken links are skipped. Links into other parts of Miyu's data folder (the local token, the secrets file and the like) don't count, and `miyu check` says where they point.

```text
dice/
├── package.toml          # only [package]
├── scripts/
│   └── dice.py
├── skills/
│   └── take-notes/
│       └── SKILL.md
└── commands/
    └── review.md
```

```toml
[package]
name = { en = "Dice and notes", zh = "骰子和笔记" }
```

## Scripts

One file, one tool. The tool name is the file name (extension removed, characters outside `[A-Za-z0-9_-]` become `_`); a comment header gives the description and parameters. A working example, `dice.py`:

```python
#!/usr/bin/env python3
# Display name: Dice
# 显示名称：掷骰子
# Description: Roll dice and return each face and the total.
# 描述：掷骰子，交回每一颗和总数。
# Timeout: 10
# Parameters:
# {"type": "object", "properties": {"count": {"type": "integer"}, "sides": {"type": "integer"}}, "required": ["count", "sides"]}
import json
import random
import sys

args = json.load(sys.stdin)
count = max(1, min(int(args["count"]), 100))
sides = max(2, min(int(args["sides"]), 1000))
faces = [random.randint(1, sides) for _ in range(count)]
print(f"{count}d{sides}: {faces} = {sum(faces)}")
```

On Linux and macOS, `chmod +x dice.py`.

**The header**

| Key | Notes |
|---|---|
| `Description` | The description for the model, short English, required |
| `描述` | The description for people (Chinese) |
| `Display name`, `显示名称` | The name for people (English, Chinese); the timeline shows it |
| `Parameters` | The parameters' JSON Schema, from the next line on, may span lines; none when omitted |
| `Timeout` | Seconds, default 60 |
| `Need load` | Loaded on demand unless you write it: the AI normally sees only the name and one line and loads the full parameters when needed, which saves money. `no`: always send the full parameters; write it for ones used often |
| `Clicall` | `yes`: the AI can also call it with `miyu call` from a script it runs, and the result goes to the script. Unset means it cannot, and it can only be called directly. Scripts that read or look things up, and that the AI can't easily do from a command on its own, should set it: the AI can call them in bulk from a script and skip many round trips. Don't set it on scripts that change or send things or spend quota |
| `Venues` | Which sessions get it: some of `local` (this machine: the terminal, the web page and so on), `private` (private chats on a messaging platform), `group` (groups), comma separated, e.g. `Venues: local, private`. Unset means every kind of session; if it is wrong the whole script is not registered and `miyu check` reports it |

The header is the run of comment lines at the very top of the file; it ends at the first line that is not a comment. Leave a blank line after the header before any other comment, or a sentence like "Timeout: …" in your notes is read as part of the header.

**How it runs**

- Started by its `#!` line (on Windows, the interpreter is found by extension: `.py` uses `python`, `.ps1` PowerShell, `.js` `node`).
- The arguments are one JSON object. Stdin always has all of it; the environment variable `MIYU_ARGS_JSON` also has it when it is at most 30 KiB. Long arguments are only on stdin, so reading stdin is the safe way.
- Stdout and stderr both go to the AI, in the order they were written; a non-zero exit code counts as failure, so write the reason to the output.
- To report progress on a long run, print a whole line starting with `miyu-progress: `: people see it in the interface, the AI doesn't, and it stays out of the result. Write it to stderr and flush (Python buffers stdout in blocks when it goes to a pipe, so progress would only show up at the end):

  ```python
  import sys
  print("miyu-progress: 3/10 downloaded", file=sys.stderr, flush=True)
  ```

- `miyu call` doesn't work inside a script tool: the channel for calling tools is only opened for a `shell` command the AI runs directly, not for script tools, so calls don't nest. If your script needs to call tools and then work on the results, don't make it a script tool; have the AI run it from `shell` (for example, ship it in a skill and say in the body how to run it).
- The working directory is the session's workspace; a writable cache directory is in `MIYU_SCRIPT_CACHE_DIR` (in your home's `cache/`, one folder per script, kept across reboots; a folder untouched for 30 days is cleared, so keep only what can be made again).
- Files meant for people (images you draw, videos you download, documents you make) go into the folders in your home, where people find them in the terminal and web UI: `MIYU_PICTURES_DIR`, `MIYU_VIDEOS_DIR`, `MIYU_DOCUMENTS_DIR`, `MIYU_DOWNLOADS_DIR`, created and writable before the script runs. Only your own sessions on this machine get them; group chats and strangers' private chats don't, so write to the cache folder or the workspace there, and check that a variable is set before using it.
- Anything that must last (a login, where the last run stopped) goes in `MIYU_SCRIPT_STATE_DIR`: one folder per script, still there next time. Local sessions share one; each group and each stranger's private chat gets its own (split by venue, so a new session in the same group finds the same folder). For a script in a package, it is deleted when the package is removed; for one you put in `scripts/` yourself, it stays after you delete the script, and `miyu check` points it out.
- Who called: `MIYU_CALLER_VENUE` is the kind of session it was called in (`local` on this machine, the venue id for a group or private chat, e.g. `qq:group:123`); `MIYU_CALLER_OWNER` says whether the person speaking this turn is the machine's owner (`1` or `0`; always `1` in local sessions; `0` when one of several people in a group is not the owner, or when it can't be told).
- The model this turn uses: `MIYU_MODEL` is the model name and `MIYU_EFFORT` the reasoning effort (`default` when the config doesn't set one); `MIYU_SESSION_LOG` is this session's log directory, only in the machine owner's own sessions (other people don't get to see the computer Miyu runs on). Calls before a model is settled, and calls outside a session, have none of these, so check before using them.
- These two are facts, not a gate: they can't stop the AI from writing `MIYU_CALLER_OWNER=1 ./script` in a command itself. To keep a script to some kinds of session, write `Venues` in the header; use the variables only to adjust what the script does, for example read-only in groups:

  ```python
  import os
  if os.environ.get("MIYU_CALLER_OWNER") != "1":
      print("Read-only here: only the owner can change the title.")
  ```

- It runs under the same rules as commands the AI runs: in the sandbox (can only write the workspace, can't read Miyu's data directory, the script file itself is readable); without a sandbox, a person must approve.

**Handing over images**

To let the AI see images directly (screenshots, frames from a video), copy them into `MIYU_SCRIPT_IMAGES_DIR`: a new empty folder for each call, which Miyu collects after the run and then deletes.

```python
import os, shutil
shutil.copy("frame-01.png", os.environ["MIYU_SCRIPT_IMAGES_DIR"])
print("Grabbed 1 frame.")
```

- Taken in file-name order, at most 4; PNG, JPEG, GIF or WebP, each at most 5 MiB and at most 8000 pixels per side.
- Only regular files directly in the folder count, and links are not followed: copy the images in, don't link them.
- Anything that doesn't fit is not handed over and the AI doesn't learn of it; the run log has a `script image dropped` line saying which script and why (not the file name).
- A call stopped at its timeout hands over no images; a non-zero exit code still does.

**Scripts that must run outside the sandbox**

Scripts that touch hardware or need root (`sudo`) don't work in the sandbox. A script can't ask for this itself, whatever its header says: you grant it in the config `config.toml`, and the value is where the tool comes from.

```toml
[sandbox.tools]
ceiling-light = "~home"   # a script you put in scripts/
lights = "smart-home"     # from a package: the package's id
```

- A granted tool runs outside the sandbox without asking in your own sessions (terminal, web, your QQ private chat, desktop voice). Group chats and strangers' private chats still refuse it, and so does read-only; dangerous commands still ask.
- A grant whose source doesn't match doesn't count: delete the script, or let another package bring a tool with the same name, and the grant lapses. `miyu check` warns about each grant whose tool is gone or whose source doesn't match, printing the current source for the latter, and also flags a grant for `shell`.
- A granted tool has all of your own permissions: only grant scripts whose code you have read and whose arguments are safe.
- A grant only counts for a real file kept in `scripts/` in your home folder (or installed as a package): if the file is a link, if `scripts/` itself is a link, or if the file sits somewhere the AI can write this time (such as the workspace), it isn't granted and still runs in the sandbox (the AI could change those). `miyu check` reminds you and prints where the file really is.

## Skills

The public Agent Skills format (Claude Code, Codex and others read it too), so existing skills from the web just work. Turn on "Read skills from other folders" (`skills.external.enabled`) and skills other clients install into `~/.agents/skills/` are picked up too (the folder list `skills.external.dirs` can be edited), after your own and those from packages; on a name clash Miyu's own wins, and Miyu does not delete anything there. `skills/<name>/SKILL.md`:

```markdown
---
name: take-notes
description: "Keep meeting notes in notes/YYYY-MM-DD.md: decisions, owners, open questions. Use when the user asks to take or tidy notes, 记笔记."
metadata:
  display_name: Take notes
  summary: Meeting notes in a fixed format
  display_name_zh: 记笔记
  summary_zh: 照固定格式记会议笔记
---

# Taking notes

1. Create `notes/` in the workspace if it doesn't exist.
2. …
```

| Field | Notes |
|---|---|
| `name` | Lowercase letters, digits, `-`; same as the folder name (a different name is used as written, a missing or invalid one falls back to the folder name; `miyu check` warns about each) |
| `description` | For the model: English, what it is and when to use it; may include a few trigger words people actually say in other languages. Quote it (an unquoted "colon space" inside is not valid YAML: Miyu reads it again as plain text and warns, other tools may not read it at all); at most 1024 characters, the rest is cut |
| `metadata` `display_name`, `summary` | Name and one line for people (English); Chinese and Japanese in `display_name_zh`, `summary_zh`, `display_name_ja`, `summary_ja` |
| `disable-model-invocation` | `true`: only a person can invoke it; the AI doesn't see it and won't pick it on its own (releases, deleting things, whatever she shouldn't start by herself) |
| `user-invocable` | `false`: a person can't invoke it and it stays out of the command menu; only the AI uses it (background knowledge and the like) |

The AI normally sees only the name, the description and the path of `SKILL.md`, and reads the body when it's useful. A person can also type `/skill-name what follows` in the input box to invoke a skill: its body and what follows go straight to her. A skill named like a command loses to the command, and `miyu check` says so. It can read other files in the folder (references, bundled scripts) but not change them; bundled scripts aren't tools, so the body says how to run them.

## Command templates

Claude Code's format: `commands/<name>.md`. When a person types `/<name> args`, the body is sent to the AI as that person's message.

```markdown
---
description: Review the changes in the working tree
argument-hint: "[focus]"
metadata:
  summary_zh: 检查工作区里的改动
---

Review the uncommitted changes in this repository. Focus on: $ARGUMENTS
```

- `$ARGUMENTS` becomes everything after the name, `$ARGUMENTS[0]` the first word (split on whitespace; a value with spaces goes in English double quotes, so for `/review "my file.rs"` `$ARGUMENTS[0]` is `my file.rs`); if the body doesn't use `$ARGUMENTS`, the arguments are appended.
- `description` is the line in the command menu (English); Chinese and Japanese go in `metadata` as `summary_zh`, `summary_ja`. `argument-hint` hints at what follows the name.
- Names: start with a lowercase letter; only lowercase letters, digits, `-`. A template with the same name as a Miyu built-in command doesn't count (the built-in wins).
- The timeline shows the line the person typed. Who can use it is the same as for built-in commands.

For slash commands handled by a program (looking something up, changing state, replying with text), use an extension's `[[slash]]`; see [Writing an extension](extensions.en.md).
