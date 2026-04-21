# Explore the .claude directory
## ​Explore the directory
## ​What’s not shown
## ​Choose the right file
## ​File reference
## ​Troubleshoot configuration
## ​Check what loaded
## ​Application data
## ​Related resources









Where Claude Code reads CLAUDE.md, settings.json, hooks, skills, commands, subagents, rules, and auto memory. Explore the .claude directory in your project and ~/.claude in your home directory.

Claude Code reads instructions, settings, skills, subagents, and memory from your project directory and from `~/.claude` in your home directory. Commit project files to git to share them with your team; files in `~/.claude` are personal configuration that applies across all your projects.
On Windows, `~/.claude` resolves to `%USERPROFILE%\.claude`. If you set [`CLAUDE_CONFIG_DIR`](https://code.claude.com/docs/en/env-vars), every `~/.claude` path on this page lives under that directory instead.
Most users only edit `CLAUDE.md` and `settings.json`. The rest of the directory is optional: add skills, rules, or subagents as you need them.


## [​](https://code.claude.com/docs/en/claude-directory#explore-the-directory) Explore the directory


Click files in the tree to see what each one does, when it loads, and an example.


## [​](https://code.claude.com/docs/en/claude-directory#what%E2%80%99s-not-shown) What’s not shown


The explorer covers files you author and edit. A few related files live elsewhere:


| File | Location | Purpose |
| --- | --- | --- |
| `managed-settings.json` | System-level, varies by OS | Enterprise-enforced settings that you can’t override. See [server-managed settings](https://code.claude.com/docs/en/server-managed-settings). |
| `CLAUDE.local.md` | Project root | Your private preferences for this project, loaded alongside CLAUDE.md. Create it manually and add it to `.gitignore`. |
| Installed plugins | `~/.claude/plugins` | Cloned marketplaces, installed plugin versions, and per-plugin data, managed by `claude plugin` commands. Orphaned versions are deleted 7 days after a plugin update or uninstall. See [plugin caching](https://code.claude.com/docs/en/plugins-reference#plugin-caching-and-file-resolution). |


`~/.claude` also holds data Claude Code writes as you work: transcripts, prompt history, file snapshots, caches, and logs. See [application data](https://code.claude.com/docs/en/claude-directory#application-data) below.


## [​](https://code.claude.com/docs/en/claude-directory#choose-the-right-file) Choose the right file


Different kinds of customization live in different files. Use this table to find where a change belongs.


| You want to | Edit | Scope | Reference |
| --- | --- | --- | --- |
| Give Claude project context and conventions | `CLAUDE.md` | project or global | [Memory](https://code.claude.com/docs/en/memory) |
| Allow or block specific tool calls | `settings.json` `permissions` or `hooks` | project or global | [Permissions](https://code.claude.com/docs/en/permissions), [Hooks](https://code.claude.com/docs/en/hooks) |
| Run a script before or after tool calls | `settings.json` `hooks` | project or global | [Hooks](https://code.claude.com/docs/en/hooks) |
| Set environment variables for the session | `settings.json` `env` | project or global | [Settings](https://code.claude.com/docs/en/settings#available-settings) |
| Keep personal overrides out of git | `settings.local.json` | project only | [Settings scopes](https://code.claude.com/docs/en/settings#settings-files) |
| Add a prompt or capability you invoke with `/name` | `skills/<name>/SKILL.md` | project or global | [Skills](https://code.claude.com/docs/en/skills) |
| Define a specialized subagent with its own tools | `agents/*.md` | project or global | [Subagents](https://code.claude.com/docs/en/sub-agents) |
| Connect external tools over MCP | `.mcp.json` | project only | [MCP](https://code.claude.com/docs/en/mcp) |
| Change how Claude formats responses | `output-styles/*.md` | project or global | [Output styles](https://code.claude.com/docs/en/output-styles) |


## [​](https://code.claude.com/docs/en/claude-directory#file-reference) File reference


This table lists every file the explorer covers. Project-scope files live in your repo under `.claude/` (or at the root for `CLAUDE.md`, `.mcp.json`, and `.worktreeinclude`). Global-scope files live in `~/.claude/` and apply across all projects.
Several things can override what you put in these files:

- [Managed settings](https://code.claude.com/docs/en/server-managed-settings) deployed by your organization take precedence over everything
- CLI flags like `--permission-mode` or `--settings` override `settings.json` for that session
- Some environment variables take precedence over their equivalent setting, but this varies: check the [environment variables reference](https://code.claude.com/docs/en/env-vars) for each one

See [settings precedence](https://code.claude.com/docs/en/settings#settings-precedence) for the full order.
Click a filename to open that node in the explorer above.


| File | Scope | Commit | What it does | Reference |
| --- | --- | --- | --- | --- |
| [`CLAUDE.md`](https://code.claude.com/docs/en/claude-directory#ce-claude-md) | Project and global | ✓ | Instructions loaded every session | [Memory](https://code.claude.com/docs/en/memory) |
| [`rules/*.md`](https://code.claude.com/docs/en/claude-directory#ce-rules) | Project and global | ✓ | Topic-scoped instructions, optionally path-gated | [Rules](https://code.claude.com/docs/en/memory#organize-rules-with-claude/rules/) |
| [`settings.json`](https://code.claude.com/docs/en/claude-directory#ce-settings-json) | Project and global | ✓ | Permissions, hooks, env vars, model defaults | [Settings](https://code.claude.com/docs/en/settings) |
| [`settings.local.json`](https://code.claude.com/docs/en/claude-directory#ce-settings-local-json) | Project only |  | Your personal overrides, auto-gitignored | [Settings scopes](https://code.claude.com/docs/en/settings#settings-files) |
| [`.mcp.json`](https://code.claude.com/docs/en/claude-directory#ce-mcp-json) | Project only | ✓ | Team-shared MCP servers | [MCP scopes](https://code.claude.com/docs/en/mcp#mcp-installation-scopes) |
| [`.worktreeinclude`](https://code.claude.com/docs/en/claude-directory#ce-worktreeinclude) | Project only | ✓ | Gitignored files to copy into new worktrees | [Worktrees](https://code.claude.com/docs/en/common-workflows#copy-gitignored-files-to-worktrees) |
| [`skills/<name>/SKILL.md`](https://code.claude.com/docs/en/claude-directory#ce-skills) | Project and global | ✓ | Reusable prompts invoked with `/name` or auto-invoked | [Skills](https://code.claude.com/docs/en/skills) |
| [`commands/*.md`](https://code.claude.com/docs/en/claude-directory#ce-commands) | Project and global | ✓ | Single-file prompts; same mechanism as skills | [Skills](https://code.claude.com/docs/en/skills) |
| [`output-styles/*.md`](https://code.claude.com/docs/en/claude-directory#ce-output-styles) | Project and global | ✓ | Custom system-prompt sections | [Output styles](https://code.claude.com/docs/en/output-styles) |
| [`agents/*.md`](https://code.claude.com/docs/en/claude-directory#ce-agents) | Project and global | ✓ | Subagent definitions with their own prompt and tools | [Subagents](https://code.claude.com/docs/en/sub-agents) |
| [`agent-memory/<name>/`](https://code.claude.com/docs/en/claude-directory#ce-agent-memory) | Project and global | ✓ | Persistent memory for subagents | [Persistent memory](https://code.claude.com/docs/en/sub-agents#enable-persistent-memory) |
| [`~/.claude.json`](https://code.claude.com/docs/en/claude-directory#ce-claude-json) | Global only |  | App state, OAuth, UI toggles, personal MCP servers | [Global config](https://code.claude.com/docs/en/settings#global-config-settings) |
| [`projects/<project>/memory/`](https://code.claude.com/docs/en/claude-directory#ce-global-projects) | Global only |  | Auto memory: Claude’s notes to itself across sessions | [Auto memory](https://code.claude.com/docs/en/memory#auto-memory) |
| [`keybindings.json`](https://code.claude.com/docs/en/claude-directory#ce-keybindings) | Global only |  | Custom keyboard shortcuts | [Keybindings](https://code.claude.com/docs/en/keybindings) |


## [​](https://code.claude.com/docs/en/claude-directory#troubleshoot-configuration) Troubleshoot configuration


If a setting, hook, or file isn’t taking effect, scan the symptoms below.


| Symptom | Cause | Fix |
| --- | --- | --- |
| Hook never fires | `matcher` is a JSON array instead of a string | Use a single string with `|` to match multiple tools, for example `"Edit|Write"`. See [matcher patterns](https://code.claude.com/docs/en/hooks#matcher-patterns). |
| Hook never fires | `matcher` value is lowercase, for example `"bash"` | Matching is case-sensitive. Tool names are capitalized: `Bash`, `Edit`, `Write`, `Read`. |
| Hook never fires | Hooks are in a standalone `.claude/hooks.json` file | There is no standalone hooks file. Define hooks under the `"hooks"` key in `settings.json`. See [hook configuration](https://code.claude.com/docs/en/hooks). |
| Permissions, hooks, or env set globally are ignored | Configuration was added to `~/.claude.json` | `~/.claude.json` holds app state and UI toggles. `permissions`, `hooks`, and `env` belong in `~/.claude/settings.json`. These are two different files. |
| A `settings.json` value seems ignored | The same key is set in `settings.local.json` | `settings.local.json` overrides `settings.json`, and both override `~/.claude/settings.json`. See [settings precedence](https://code.claude.com/docs/en/settings#settings-precedence). |
| Skill doesn’t appear in `/skills` | Skill file is at `.claude/skills/name.md` instead of in a folder | Use a folder with `SKILL.md` inside: `.claude/skills/name/SKILL.md`. |
| Subdirectory `CLAUDE.md` instructions seem ignored | Subdirectory files load on demand, not at session start | They load when Claude reads a file in that directory with the Read tool, not at launch and not when writing or creating files there. See [how CLAUDE.md files load](https://code.claude.com/docs/en/memory#how-claude-md-files-load). |
| Subagent ignores `CLAUDE.md` instructions | Subagents don’t always inherit project memory | Put critical rules in the agent file body, which becomes the subagent’s system prompt. See [subagent configuration](https://code.claude.com/docs/en/sub-agents). |
| Cleanup logic never runs at session end | No `SessionEnd` hook configured | `SessionStart` and `SessionEnd` both exist. See the [hook events list](https://code.claude.com/docs/en/hooks#hook-events). |
| MCP servers in `.mcp.json` never load | File is under `.claude/` or uses Claude Desktop’s config format | Project MCP config lives at the repository root as `.mcp.json`, not inside `.claude/`. See [MCP configuration](https://code.claude.com/docs/en/mcp). |
| Project MCP server added but doesn’t appear | The one-time approval prompt was dismissed | Project-scoped servers require approval. Run `/mcp` to see status and approve. |
| MCP server fails to start from some directories | `command` or `args` uses a relative file path | Use absolute paths for local scripts. Executables on your `PATH` like `npx` or `uvx` work as-is. |
| MCP server starts without expected environment variables | Variables are in `settings.json` `env`, which doesn’t propagate to MCP child processes | Set per-server `env` inside `.mcp.json` instead. |
| `Bash(rm *)` deny rule doesn’t block `/bin/rm` or `find -delete` | Prefix rules match the literal command string, not the underlying executable | Add explicit patterns for each variant, or use a [PreToolUse hook](https://code.claude.com/docs/en/hooks-guide) or the [sandbox](https://code.claude.com/docs/en/sandboxing) for a hard guarantee. |


## [​](https://code.claude.com/docs/en/claude-directory#check-what-loaded) Check what loaded


The explorer shows what files can exist. To see what actually loaded in your current session, use these commands:


| Command | Shows |
| --- | --- |
| `/context` | Token usage by category: system prompt, memory files, skills, MCP tools, and messages |
| `/memory` | Which CLAUDE.md and rules files loaded, plus auto-memory entries |
| `/agents` | Configured subagents and their settings |
| `/hooks` | Active hook configurations |
| `/mcp` | Connected MCP servers and their status |
| `/skills` | Available skills from project, user, and plugin sources |
| `/permissions` | Current allow and deny rules |
| `/doctor` | Installation and configuration diagnostics |


Run `/context` first for the overview, then the specific command for the area you want to investigate.


## [​](https://code.claude.com/docs/en/claude-directory#application-data) Application data


Beyond the config you author, `~/.claude` holds data Claude Code writes during sessions. These files are plaintext. Anything that passes through a tool lands in a transcript on disk: file contents, command output, pasted text.


### [​](https://code.claude.com/docs/en/claude-directory#cleaned-up-automatically) Cleaned up automatically


Files in the paths below are deleted on startup once they’re older than [`cleanupPeriodDays`](https://code.claude.com/docs/en/settings#available-settings). The default is 30 days.


| Path under `~/.claude/` | Contents |
| --- | --- |
| `projects/<project>/<session>.jsonl` | Full conversation transcript: every message, tool call, and tool result |
| `projects/<project>/<session>/tool-results/` | Large tool outputs spilled to separate files |
| `file-history/<session>/` | Pre-edit snapshots of files Claude changed, used for [checkpoint restore](https://code.claude.com/docs/en/checkpointing) |
| `plans/` | Plan files written during [plan mode](https://code.claude.com/docs/en/permission-modes#analyze-before-you-edit-with-plan-mode) |
| `debug/` | Per-session debug logs, written only when you start with `--debug` or run `/debug` |
| `paste-cache/`, `image-cache/` | Contents of large pastes and attached images |
| `session-env/` | Per-session environment metadata |


### [​](https://code.claude.com/docs/en/claude-directory#kept-until-you-delete-them) Kept until you delete them


The following paths are not covered by automatic cleanup and persist indefinitely.


| Path under `~/.claude/` | Contents |
| --- | --- |
| `history.jsonl` | Every prompt you’ve typed, with timestamp and project path. Used for up-arrow recall. |
| `stats-cache.json` | Aggregated token and cost counts shown by `/cost` |
| `backups/` | Timestamped copies of `~/.claude.json` taken before config migrations |
| `todos/` | Legacy per-session task lists. No longer written by current versions; safe to delete. |


`shell-snapshots/` holds runtime files removed when the session exits cleanly. Other small cache and lock files appear depending on which features you use and are safe to delete.


### [​](https://code.claude.com/docs/en/claude-directory#plaintext-storage) Plaintext storage


Transcripts and history are not encrypted at rest. OS file permissions are the only protection. If a tool reads a `.env` file or a command prints a credential, that value is written to `projects/<project>/<session>.jsonl`. To reduce exposure:


- Lower `cleanupPeriodDays` to shorten how long transcripts are kept
- Set the [`CLAUDE_CODE_SKIP_PROMPT_HISTORY`](https://code.claude.com/docs/en/env-vars) environment variable to skip writing transcripts and prompt history in any mode. In non-interactive mode, you can instead pass `--no-session-persistence` alongside `-p`, or set `persistSession: false` in the Agent SDK.
- Use [permission rules](https://code.claude.com/docs/en/permissions) to deny reads of credential files


### [​](https://code.claude.com/docs/en/claude-directory#clear-local-data) Clear local data


You can delete any of the application-data paths above at any time. New sessions are unaffected. The table below shows what you lose for past sessions.


| Delete | You lose |
| --- | --- |
| `~/.claude/projects/` | Resume, continue, and rewind for past sessions |
| `~/.claude/history.jsonl` | Up-arrow prompt recall |
| `~/.claude/file-history/` | Checkpoint restore for past sessions |
| `~/.claude/stats-cache.json` | Historical totals shown by `/cost` |
| `~/.claude/backups/` | Rollback copies of `~/.claude.json` from past config migrations |
| `~/.claude/debug/`, `~/.claude/plans/`, `~/.claude/paste-cache/`, `~/.claude/image-cache/`, `~/.claude/session-env/` | Nothing user-facing |
| `~/.claude/todos/` | Nothing. Legacy directory not written by current versions. |


Don’t delete `~/.claude.json`, `~/.claude/settings.json`, or `~/.claude/plugins/`: those hold your auth, preferences, and installed plugins.


## [​](https://code.claude.com/docs/en/claude-directory#related-resources) Related resources


- [Manage Claude’s memory](https://code.claude.com/docs/en/memory): write and organize CLAUDE.md, rules, and auto memory
- [Configure settings](https://code.claude.com/docs/en/settings): set permissions, hooks, environment variables, and model defaults
- [Create skills](https://code.claude.com/docs/en/skills): build reusable prompts and workflows
- [Configure subagents](https://code.claude.com/docs/en/sub-agents): define specialized agents with their own context[Claude Code Docs home page](https://code.claude.com/docs/en/overview)

[Privacy choices](https://code.claude.com/docs/en/claude-directory#)

