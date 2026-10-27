# Errors

A refusal:

```json
{"error": {"code": -32010, "data": {"reason": "session_not_found"}, "message": "Session not found."}, "id": "c7", "jsonrpc": "2.0"}
```

- `code`: JSON-RPC's own errors use the standard codes (the first five rows below); Miyu's are all `-32010`.
- `data.reason`: the reason code for programs; branch on it. Some carry more fields, listed in the tables.
- `message`: one sentence for people in the connection's language (in English, or the language given in that `hello`, before and during the handshake). Don't branch on it.

Chinese version: [errors.md](errors.md).

## General

| `reason` | `code` | When |
|---|---|---|
| `parse_error` | -32700 | Not JSON; a line too long (then closes) |
| `invalid_request` | -32600 | JSON but not a request |
| `unknown_method` | -32601 | No such method (also methods of optional packages not compiled in) |
| `bad_params` | -32602 | Params can't be read, have the wrong type or form; each method's "Errors" lists its cases |
| `internal_error` | -32603 | Something went wrong inside the core (disk, a broken file…); the reason is in the core's log |
| `hello_first` | -32010 | Another method before the handshake |
| `local_only` | -32010 | A method for people only, called by an extension; or the wrong way of logging in |
| `local_machine_only` | -32010 | Only on the machine running the core: a remote connection, or another person on this machine, called something that reads local files or takes local paths (Y-1 下, [Connection](connection.en.md) "Local machine only") |
| `not_a_provider`, `not_an_extension` | -32010 | A method only for extensions the core launched, called by another connection |
| `needs_approval` | -32010 | Capabilities the extension needs aren't approved yet, or the method it called needs one its manifest doesn't declare, `data.capabilities` |
| `no_system_account` | -32010 | A method only for system account connections |

## Connecting and logging in

`protocol_mismatch`, `bad_token`, `bad_code`, `bad_login`, `bad_password`, `account_disabled`, `bad_invite`, `login_throttled` (all close the connection after answering), `setup_first`, `register_first`, `not_permitted`, `admin_mode_required` (an admin changing the system layer without `"scope": "system"`); `username_taken`, `unknown_invite`, `unknown_account`, `member_disabled` (the member being made an admin is disabled) from the member methods; `bad_ticket` when the ticket for `login.redeem` doesn't exist, has expired or was already used. See [Connection](connection.en.md).

## Sessions

| `reason` | When | `data` |
|---|---|---|
| `session_not_found` | No such session; deleted ones too | |
| `session_broken` | The session can't be loaded: its log is broken or unreadable | |
| `session_stopped` | The session stopped; sending again reloads it | |
| `session_sealed` | The session is sealed; commands that change the conversation are refused | |
| `restoring` | A command arrived while an undo is still finishing | |
| `venue_session` | Speaking into a venue session without `as` | |
| `empty_message` | A message with no content at all | |
| `dir_too_wide` | An extra directory is too wide | |
| `not_running` | Interrupting with no turn running | |
| `nothing_queued`, `already_heard` | Taking back a queued message (`session.withdraw`): nothing queued; a request already took the named one | |
| `turn_running` | Undo, redo, compact, clear, delete, or change persona or preset while a turn is running | |
| `unknown_turn` | The turn to undo isn't in the current history | |
| `nothing_to_revert`, `nothing_to_unrevert` | Nothing to undo; nothing to restore | |
| `not_redoable` | The last turn wasn't started by a person | |
| `nothing_to_compact`, `nothing_to_clear` | Nothing to compact; the context is already empty | |
| `nothing_to_recap`, `recap_failed` | Nothing to recap; the recap wasn't written | |
| `not_asking` | The call being answered isn't waiting for an answer | |
| `no_rule`, `unexpected_reason`, `bad_answer` | An approval or answer doesn't fit the request | |
| `not_ambient`, `already_answered` | `session.respond`'s `to` is wrong | `messages` |
| `unknown_call` | `view.detail` can't find that call or compaction | |
| `unknown_job`, `not_a_command` | No such job; reading the output of a subagent | |
| `goal_exists`, `no_goal`, `goal_paused`, `goal_running`, `goal_unavailable` | A goal command doesn't fit the goal's state; this session has no standing goal | `why` |

## Attachments and files

| `reason` | When | `data` |
|---|---|---|
| `attachment_unreadable`, `attachment_in_data_root` | `blob.put`'s path can't be read; it's in the data directory | |
| `attachment_too_big` | Over the limit | |
| `unknown_attachment` | An attachment's blob doesn't exist | |
| `unknown_blob` | No such blob, or this connection can't read it | |
| `too_many_uploads`, `upload_unknown` | Too many chunked uploads open; unknown upload id | |
| `upload_offset`, `upload_incomplete` | Wrong chunk offset; not everything arrived | `received` |
| `path_unreadable`, `path_forbidden`, `not_a_directory` | A path doesn't exist or can't be read; it's in the data directory; it's a file, not a directory | |

## Config, secrets, models

| `reason` | When | `data` |
|---|---|---|
| `unknown_config_key` | No such key in the settings list | `problems` |
| `config_invalid` | A bad value, or not allowed in this layer | `problems` |
| `config_conflict` | `expect` or `version` doesn't match | `current` or `version` |
| `config_file_broken` | The file can't be read now, so single settings can't be changed | `problems` |
| `no_project_config` | No project config from this directory | |
| `unknown_file` | `check`'s file isn't one Miyu reads | |
| `unknown_secret` | No such secret | |
| `unknown_provider`, `unknown_model` | Not a configured provider; the model can't be resolved | |
| `provider_exists` | Renaming a provider (`provider.rename`): the new ID already exists | |
| `no_model` | No usable model | `message` |
| `cooling` | Every candidate is cooling down | `message`, `wait_ms` |
| `model_failed` | The request was sent and failed | `class`, `status`, `message` |
| `embedding_off`, `embedding_failed` | The embedding model is off; couldn't compute | `why` |

## Personas and presets

| `reason` | When | `data` |
|---|---|---|
| `unknown_persona`, `unknown_preset` | No such persona or preset | |
| `persona_invalid`, `preset_invalid` | Its files are wrong | `problem`, `message`, `line` |
| `persona_conflict`, `preset_conflict` | `expect` doesn't match | `current` |
| `nothing_to_delete` | You have no layer to delete | |
| `avatar_not_image`, `avatar_too_big`, `background_not_image`, `background_too_big` | The avatar or background isn't an image, or is too big | measured size and limit |

## Projects

| `reason` | When | `data` |
|---|---|---|
| `unknown_schedule` | This account has no such schedule: `schedule.cancel`, `schedule.pause`, `schedule.resume` (construction J-1 lower) | |
| `too_many_schedules` | This account already has 200 schedules (added by people, the AI and packages together; a safety limit): `schedule.add` (construction J-3 upper). Remove some first; existing ones keep running | |
| `unknown_project` | No such project: the project methods, and `project` in `session.create` and `session.set_meta` | |

## Packages, extensions, commands

| `reason` | When | `data` |
|---|---|---|
| `unknown_package` | No such package | |
| `package_invalid` | The manifest is wrong | `problem`, `line` |
| `package_conflict`, `package_exists`, `package_id_taken` | A name it brings clashes; same id as a built-in; id already taken by an account | |
| `package_file_bad`, `wrong_target` | The package file can't be unpacked; the manifest lists `targets` and this system isn't one of them | |
| `download_failed`, `source_unreachable` | Installing from a source: the package file couldn't be downloaded from anywhere; no source could be reached and there is no local index | `why` |
| `depends_missing`, `package_depended` | A dependency is missing; another package depends on it | `missing`, `by` |
| `program_missing`, `not_switchable` | The program wasn't found; no switch | |
| `not_restartable`, `package_off` | Nothing to restart; it's off | |
| `extension_off` | Restarting an extension that's off | |
| `port_taken` | Another extension has claimed the port (`port.claim`); it doesn't say which | |
| `unknown_server` | No such MCP server in personal settings | |
| `oauth_unregistered`, `oauth_failed` | `mcp.login`: no client and no dynamic registration; the login can't go through (`mcp.login_finish`: the pasted address doesn't match) | `why` |
| `not_logged_in` | `mcp.logout`: this server was never logged in | |
| `not_waiting` | `mcp.login_finish`: no login is waiting for this server | |
| `mcp_invalid` | The server sent to `mcp.set` is badly written | `problem` |
| `server_from_package` | `mcp.set`: personal settings have no server by that name and a package brings one | |
| `unknown_skill` | `skill.remove`: no such skill | `name` |
| `skill_from_package` | `skill.remove`: a skill that comes with a package cannot be deleted | `name` |
| `no_page`, `not_found` | No package page; no such file | |
| `program_not_running`, `unregistered`, `method_failed`, `method_timeout` | `package.call`: the program isn't connected; not registered; it answered with an error; no answer | `method`, `message`, `code` |
| `not_exported` | An extension called a method the other package doesn't export | `method` |
| `bad_tool` | A tool in `provide` is wrong | `tool`, `problem` |
| `unknown_widget`, `bad_widget_content` | The widget isn't in the manifest; the content is malformed | `widget`; `path`, `problem` |
| `unknown_command`, `command_not_allowed`, `owner_only` | No such command; this person can't use it; owner only | `why` |
| `command_unavailable`, `command_failed` | An extension's command or an MCP server's prompt can't be used right now, or failed; which cases are which is in [`command.run`](packages.en.md#commandrun) | `why` |

## Memory and knowledge bases

| `reason` | When | `data` |
|---|---|---|
| `memory_not_installed`, `memory_unavailable` | Memory isn't installed; there's no memory here | `why` |
| `memory_busy`, `dream_failed` | Already organizing; organizing failed | `message` |
| `not_dreaming` | `/dream stop`, `memory.dream_stop`: nothing is organizing | |
| `unknown_memory`, `memory_not_current` | No such memory; already changed, retired or cleared | |
| `memory_too_long` | Over 120 characters | `chars`, `limit` |
| `unknown_kb`, `unknown_document` | No such knowledge base or document | |
| `kb_exists`, `document_exists`, `kb_read_only`, `kb_packaged` | Already exists; mounted ones can't be added to or removed from; ones from packages can't be changed | |
| `folder_unavailable`, `trash_failed` | The folder to mount can't be used; couldn't move it to the trash | `message` |

## Diagrams

`mermaid_too_long`, `mermaid_failed` (`detail`).
