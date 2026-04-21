# Claude Code overview
## ​Get started
## ​What you can do
## ​Use Claude Code everywhere
## ​Next steps





Claude Code is an agentic coding tool that reads your codebase, edits files, runs commands, and integrates with your development tools. Available in your terminal, IDE, desktop app, and browser.

Claude Code is an AI-powered coding assistant that helps you build features, fix bugs, and automate development tasks. It understands your entire codebase and can work across multiple files and tools to get things done.


## [​](https://code.claude.com/docs/en/overview#get-started) Get started


Choose your environment to get started. Most surfaces require a [Claude subscription](https://claude.com/pricing?utm_source=claude_code&utm_medium=docs&utm_content=overview_pricing) or [Anthropic Console](https://console.anthropic.com/) account. The Terminal CLI and VS Code also support [third-party providers](https://code.claude.com/docs/en/third-party-integrations).


- Terminal
- VS Code
- Desktop app
- Web
- JetBrains

The full-featured CLI for working with Claude Code directly in your terminal. Edit files, run commands, and manage your entire project from the command line. To install Claude Code, use one of the following methods:

- Native Install (Recommended)
- Homebrew
- WinGet

**macOS, Linux, WSL:**

```
curl -fsSL https://claude.ai/install.sh | bash
```

**Windows PowerShell:**

```
irm https://claude.ai/install.ps1 | iex
```

**Windows CMD:**

```
curl -fsSL https://claude.ai/install.cmd -o install.cmd && install.cmd && del install.cmd
```

If you see `The token '&&' is not a valid statement separator`, you’re in PowerShell, not CMD. If you see `'irm' is not recognized as an internal or external command`, you’re in CMD, not PowerShell. Your prompt shows `PS C:\` when you’re in PowerShell and `C:\` without the `PS` when you’re in CMD. **Native Windows setups require [Git for Windows](https://git-scm.com/downloads/win).** Install it first if you don’t have it. WSL setups do not need it. Native installations automatically update in the background to keep you on the latest version.

```
brew install --cask claude-code
```

Homebrew offers two casks. `claude-code` tracks the stable release channel, which is typically about a week behind and skips releases with major regressions. `claude-code@latest` tracks the latest channel and receives new versions as soon as they ship. Homebrew installations do not auto-update. Run `brew upgrade claude-code` or `brew upgrade claude-code@latest`, depending on which cask you installed, to get the latest features and security fixes.

```
winget install Anthropic.ClaudeCode
```

WinGet installations do not auto-update. Run `winget upgrade Anthropic.ClaudeCode` periodically to get the latest features and security fixes. Then start Claude Code in any project:

```
cd your-project
claude
```

You’ll be prompted to log in on first use. That’s it! [Continue with the Quickstart →](https://code.claude.com/docs/en/quickstart) See [advanced setup](https://code.claude.com/docs/en/setup) for installation options, manual updates, or uninstallation instructions. Visit [troubleshooting](https://code.claude.com/docs/en/troubleshooting) if you hit issues. The VS Code extension provides inline diffs, @-mentions, plan review, and conversation history directly in your editor.

- [Install for VS Code](vscode:extension/anthropic.claude-code)
- [Install for Cursor](cursor:extension/anthropic.claude-code)

Or search for “Claude Code” in the Extensions view ( `Cmd+Shift+X` on Mac, `Ctrl+Shift+X` on Windows/Linux). After installing, open the Command Palette ( `Cmd+Shift+P` / `Ctrl+Shift+P`), type “Claude Code”, and select **Open in New Tab**. [Get started with VS Code →](https://code.claude.com/docs/en/vs-code#get-started) A standalone app for running Claude Code outside your IDE or terminal. Review diffs visually, run multiple sessions side by side, schedule recurring tasks, and kick off cloud sessions. Download and install:

- [macOS](https://claude.ai/api/desktop/darwin/universal/dmg/latest/redirect?utm_source=claude_code&utm_medium=docs) (Intel and Apple Silicon)
- [Windows](https://claude.ai/api/desktop/win32/x64/setup/latest/redirect?utm_source=claude_code&utm_medium=docs) (x64)
- [Windows ARM64](https://claude.ai/api/desktop/win32/arm64/setup/latest/redirect?utm_source=claude_code&utm_medium=docs)

After installing, launch Claude, sign in, and click the **Code** tab to start coding. A [paid subscription](https://claude.com/pricing?utm_source=claude_code&utm_medium=docs&utm_content=overview_desktop_pricing) is required. [Learn more about the desktop app →](https://code.claude.com/docs/en/desktop-quickstart) Run Claude Code in your browser with no local setup. Kick off long-running tasks and check back when they’re done, work on repos you don’t have locally, or run multiple tasks in parallel. Available on desktop browsers and the Claude iOS app. Start coding at [claude.ai/code](https://claude.ai/code). [Get started on the web →](https://code.claude.com/docs/en/web-quickstart) A plugin for IntelliJ IDEA, PyCharm, WebStorm, and other JetBrains IDEs with interactive diff viewing and selection context sharing. Install the [Claude Code plugin](https://plugins.jetbrains.com/plugin/27310-claude-code-beta-) from the JetBrains Marketplace and restart your IDE. [Get started with JetBrains →](https://code.claude.com/docs/en/jetbrains)


## [​](https://code.claude.com/docs/en/overview#what-you-can-do) What you can do


Here are some of the ways you can use Claude Code:


Automate the work you keep putting off

Claude Code handles the tedious tasks that eat up your day: writing tests for untested code, fixing lint errors across a project, resolving merge conflicts, updating dependencies, and writing release notes.

```
claude "write tests for the auth module, run them, and fix any failures"
```


Build features and fix bugs

Describe what you want in plain language. Claude Code plans the approach, writes the code across multiple files, and verifies it works. For bugs, paste an error message or describe the symptom. Claude Code traces the issue through your codebase, identifies the root cause, and implements a fix. See [common workflows](https://code.claude.com/docs/en/common-workflows) for more examples.

Create commits and pull requests

Claude Code works directly with git. It stages changes, writes commit messages, creates branches, and opens pull requests.

```
claude "commit my changes with a descriptive message"
```

In CI, you can automate code review and issue triage with [GitHub Actions](https://code.claude.com/docs/en/github-actions) or [GitLab CI/CD](https://code.claude.com/docs/en/gitlab-ci-cd).

Connect your tools with MCP

The [Model Context Protocol (MCP)](https://code.claude.com/docs/en/mcp) is an open standard for connecting AI tools to external data sources. With MCP, Claude Code can read your design docs in Google Drive, update tickets in Jira, pull data from Slack, or use your own custom tooling.

Customize with instructions, skills, and hooks

[`CLAUDE.md`](https://code.claude.com/docs/en/memory) is a markdown file you add to your project root that Claude Code reads at the start of every session. Use it to set coding standards, architecture decisions, preferred libraries, and review checklists. Claude also builds [auto memory](https://code.claude.com/docs/en/memory#auto-memory) as it works, saving learnings like build commands and debugging insights across sessions without you writing anything. Create [custom commands](https://code.claude.com/docs/en/skills) to package repeatable workflows your team can share, like `/review-pr` or `/deploy-staging`. [Hooks](https://code.claude.com/docs/en/hooks) let you run shell commands before or after Claude Code actions, like auto-formatting after every file edit or running lint before a commit.

Run agent teams and build custom agents

Spawn [multiple Claude Code agents](https://code.claude.com/docs/en/sub-agents) that work on different parts of a task simultaneously. A lead agent coordinates the work, assigns subtasks, and merges results. For fully custom workflows, the [Agent SDK](https://code.claude.com/docs/en/agent-sdk/overview) lets you build your own agents powered by Claude Code’s tools and capabilities, with full control over orchestration, tool access, and permissions.

Pipe, script, and automate with the CLI

Claude Code is composable and follows the Unix philosophy. Pipe logs into it, run it in CI, or chain it with other tools:

```
# Analyze recent log output
tail -200 app.log | claude -p "Slack me if you see any anomalies"

# Automate translations in CI
claude -p "translate new strings into French and raise a PR for review"

# Bulk operations across files
git diff main --name-only | claude -p "review these changed files for security issues"
```

See the [CLI reference](https://code.claude.com/docs/en/cli-reference) for the full set of commands and flags.

Schedule recurring tasks

Run Claude on a schedule to automate work that repeats: morning PR reviews, overnight CI failure analysis, weekly dependency audits, or syncing docs after PRs merge.

- [Routines](https://code.claude.com/docs/en/routines) run on Anthropic-managed infrastructure, so they keep running even when your computer is off. They can also trigger on API calls or GitHub events. Create them from the web, the Desktop app, or by running `/schedule` in the CLI.
- [Desktop scheduled tasks](https://code.claude.com/docs/en/desktop-scheduled-tasks) run on your machine, with direct access to your local files and tools
- [`/loop`](https://code.claude.com/docs/en/scheduled-tasks) repeats a prompt within a CLI session for quick polling


Work from anywhere

Sessions aren’t tied to a single surface. Move work between environments as your context changes:

- Step away from your desk and keep working from your phone or any browser with [Remote Control](https://code.claude.com/docs/en/remote-control)
- Message [Dispatch](https://code.claude.com/docs/en/desktop#sessions-from-dispatch) a task from your phone and open the Desktop session it creates
- Kick off a long-running task on the [web](https://code.claude.com/docs/en/claude-code-on-the-web) or [iOS app](https://apps.apple.com/app/claude-by-anthropic/id6473753684), then pull it into your terminal with `claude --teleport`
- Hand off a terminal session to the [Desktop app](https://code.claude.com/docs/en/desktop) with `/desktop` for visual diff review
- Route tasks from team chat: mention `@Claude` in [Slack](https://code.claude.com/docs/en/slack) with a bug report and get a pull request back


## [​](https://code.claude.com/docs/en/overview#use-claude-code-everywhere) Use Claude Code everywhere


Each surface connects to the same underlying Claude Code engine, so your CLAUDE.md files, settings, and MCP servers work across all of them.
Beyond the [Terminal](https://code.claude.com/docs/en/quickstart), [VS Code](https://code.claude.com/docs/en/vs-code), [JetBrains](https://code.claude.com/docs/en/jetbrains), [Desktop](https://code.claude.com/docs/en/desktop), and [Web](https://code.claude.com/docs/en/claude-code-on-the-web) environments above, Claude Code integrates with CI/CD, chat, and browser workflows:


| I want to… | Best option |
| --- | --- |
| Continue a local session from my phone or another device | [Remote Control](https://code.claude.com/docs/en/remote-control) |
| Push events from Telegram, Discord, iMessage, or my own webhooks into a session | [Channels](https://code.claude.com/docs/en/channels) |
| Start a task locally, continue on mobile | [Web](https://code.claude.com/docs/en/claude-code-on-the-web) or [Claude iOS app](https://apps.apple.com/app/claude-by-anthropic/id6473753684) |
| Run Claude on a recurring schedule | [Routines](https://code.claude.com/docs/en/routines) or [Desktop scheduled tasks](https://code.claude.com/docs/en/desktop-scheduled-tasks) |
| Automate PR reviews and issue triage | [GitHub Actions](https://code.claude.com/docs/en/github-actions) or [GitLab CI/CD](https://code.claude.com/docs/en/gitlab-ci-cd) |
| Get automatic code review on every PR | [GitHub Code Review](https://code.claude.com/docs/en/code-review) |
| Route bug reports from Slack to pull requests | [Slack](https://code.claude.com/docs/en/slack) |
| Debug live web applications | [Chrome](https://code.claude.com/docs/en/chrome) |
| Build custom agents for your own workflows | [Agent SDK](https://code.claude.com/docs/en/agent-sdk/overview) |


## [​](https://code.claude.com/docs/en/overview#next-steps) Next steps


Once you’ve installed Claude Code, these guides help you go deeper.


- [Quickstart](https://code.claude.com/docs/en/quickstart): walk through your first real task, from exploring a codebase to committing a fix
- [Store instructions and memories](https://code.claude.com/docs/en/memory): give Claude persistent instructions with CLAUDE.md files and auto memory
- [Common workflows](https://code.claude.com/docs/en/common-workflows) and [best practices](https://code.claude.com/docs/en/best-practices): patterns for getting the most out of Claude Code
- [Settings](https://code.claude.com/docs/en/settings): customize Claude Code for your workflow
- [Troubleshooting](https://code.claude.com/docs/en/troubleshooting): solutions for common issues
- [code.claude.com](https://code.claude.com/): demos, pricing, and product details[Claude Code Docs home page](https://code.claude.com/docs/en/overview)

[Privacy choices](https://code.claude.com/docs/en/overview#)

