# Terminal guide for new users
## ​macOS and Linux
## ​Windows
## ​What’s next?
## ​Troubleshooting





A step-by-step guide to installing Claude Code for first-time terminal users on macOS and Windows.

You can use Claude Code even if you’ve never used a terminal before. This guide walks you through opening a terminal, installing Claude Code, and your first interactions.


- [macOS and Linux](https://code.claude.com/docs/en/terminal-guide#macos-and-linux)
- [Windows](https://code.claude.com/docs/en/terminal-guide#windows)


Don’t want to use the terminal? The Claude Code desktop app lets you skip the terminal entirely. Download it for [macOS](https://claude.ai/api/desktop/darwin/universal/dmg/latest/redirect?utm_source=claude_code&utm_medium=docs) or [Windows](https://claude.com/download?utm_source=claude_code&utm_medium=docs), then see the [Desktop quickstart](https://code.claude.com/docs/en/desktop-quickstart) to get started.


## [​](https://code.claude.com/docs/en/terminal-guide#macos-and-linux) macOS and Linux


Follow these three steps to install and start Claude Code from a macOS or Linux terminal.
1

Open a terminal

**macOS**: Press `Cmd + Space` to open Spotlight Search, type `Terminal`, and press `Enter`. **Linux**: Open your terminal app. On most distributions, press `Ctrl + Alt + T` or search for “Terminal” in your application menu. A window will appear with a blinking cursor. This is your terminal, where you type commands. 2

Install Claude Code

Copy this line, paste it into your terminal ( `Cmd + V` on macOS, `Ctrl + Shift + V` on Linux), and press `Enter`:

```
curl -fsSL https://claude.ai/install.sh | bash
```

This downloads and runs the Claude Code installer from claude.ai. You’ll see text scrolling as it works. When it’s done, you’ll see “Claude Code successfully installed!” If you see an error instead, check the [troubleshooting section](https://code.claude.com/docs/en/terminal-guide#macos-and-linux-troubleshooting) below. 3

Start Claude Code

Type `claude` and press `Enter`:

```
claude
```

You’ll be prompted to [log in](https://code.claude.com/docs/en/authentication) with your Claude account. Follow the on-screen instructions. A browser window will open for you to sign in. 4

Start using Claude Code

Once logged in, you can start asking Claude questions about your code or anything else. Claude Code runs entirely in text. You type messages and press `Enter` to send them. A few things to know:

- You can’t click on things in the terminal. Use the arrow keys to move around.
- Press `Esc` to interrupt Claude if it’s running.
- Type `exit` or press `Ctrl + D` to leave Claude Code.
- Type `/help` to see available commands.


---


## [​](https://code.claude.com/docs/en/terminal-guide#windows) Windows


Follow these four steps to install Git, set up PowerShell, and start Claude Code on Windows.
1

Install Git for Windows

Git is a tool that Claude Code uses internally to track changes to your code. You won’t need to learn Git yourself. If you don’t already have it:

1. Go to [git-scm.com/downloads/win](https://git-scm.com/downloads/win) and download the installer
2. Run the installer. Click Next on each screen to accept the defaults. The installer has many screens, but you don’t need to change anything.
3. If it asks you to choose an editor, keep the default and click Next.
4. When you see “Adjusting your PATH environment,” keep the recommended option selected.

Already have Git? You can skip this step. If you’re not sure, install it anyway. Reinstalling won’t cause problems. 2

Open PowerShell

PowerShell is Windows’ built-in terminal for typing commands. It comes pre-installed on every Windows computer. Press `Win + X` and select **Windows PowerShell** (or **Terminal**) from the menu. A window with a blinking cursor will appear. This is where you’ll type commands. Windows has two command-line programs: PowerShell and CMD. They look similar but use different commands. Make sure you’re in PowerShell for the next step. How to tell which one you’re in:

- **PowerShell**: shows `PS C:\Users\YourName>` at the start of each line
- **CMD**: shows `C:\Users\YourName>` without the `PS`

3

Install Claude Code

Copy this line, paste it into PowerShell with `Ctrl + V` or right-click, and press `Enter`:

```
irm https://claude.ai/install.ps1 | iex
```

This downloads and runs the Claude Code installer. `irm` fetches the file and `iex` runs it. You’ll see text scrolling as it works. When it’s done, you’ll see “Claude Code successfully installed!” If you see an error instead, check the [troubleshooting section](https://code.claude.com/docs/en/terminal-guide#windows-troubleshooting) below. If you’re in CMD instead of PowerShell, use this command:

```
curl -fsSL https://claude.ai/install.cmd -o install.cmd && install.cmd && del install.cmd
```

4

Start Claude Code

Close PowerShell and open a new PowerShell window so it recognizes the newly installed `claude` command. Then type:

```
claude
```

You’ll be prompted to [log in](https://code.claude.com/docs/en/authentication) with your Claude account. Follow the on-screen instructions. A browser window will open for you to sign in. 5

Start using Claude Code

Once logged in, you can start asking Claude questions about your code or anything else. Claude Code runs entirely in text. You type messages and press `Enter` to send them. A few things to know:

- You can’t click on things in the terminal. Use the arrow keys to move around.
- Press `Esc` to interrupt Claude if it’s running.
- Type `exit` or press `Ctrl + D` to leave Claude Code.
- Type `/help` to see available commands.


---


## [​](https://code.claude.com/docs/en/terminal-guide#what%E2%80%99s-next) What’s next?


Once you see the Claude Code welcome screen, you’re ready to go. You don’t need to know how to code. Describe what you want in plain English, and Claude writes the code for you.


### [​](https://code.claude.com/docs/en/terminal-guide#build-something) Build something


Claude can create projects from a description:


```
make me a simple webpage that says hello world
```


Claude creates the files for you. Double-click the HTML file to open it in your browser.


### [​](https://code.claude.com/docs/en/terminal-guide#work-with-files-on-your-computer) Work with files on your computer


Claude can read and organize files you already have:


```
look at the screenshots on my Desktop and rename them based on what's in each image
```


### [​](https://code.claude.com/docs/en/terminal-guide#ask-questions) Ask questions


Claude can explain things, help you learn, or plan out a project:


```
I want to build a personal budget tracker. What would I need?
```


If you don’t have a project yet, that’s fine. Claude can help you start a new one.


### [​](https://code.claude.com/docs/en/terminal-guide#other-ways-to-use-claude-code) Other ways to use Claude Code


You don’t have to use the terminal. Claude Code is also available in:


- [VS Code](https://code.claude.com/docs/en/vs-code) and [JetBrains IDEs](https://code.claude.com/docs/en/jetbrains) as editor extensions
- The [desktop app](https://code.claude.com/docs/en/desktop-quickstart), with no terminal required
- The [web](https://code.claude.com/docs/en/claude-code-on-the-web) at claude.ai/code for remote sessions
- [GitHub Actions](https://code.claude.com/docs/en/github-actions) and [GitLab CI/CD](https://code.claude.com/docs/en/gitlab-ci-cd) for automation


### [​](https://code.claude.com/docs/en/terminal-guide#learn-more) Learn more


- [Quickstart](https://code.claude.com/docs/en/quickstart): a guided walkthrough of your first project with Claude Code
- [How Claude Code works](https://code.claude.com/docs/en/how-claude-code-works): understand how Claude reads your files, runs commands, and makes edits
- [Best practices](https://code.claude.com/docs/en/best-practices): get better results with effective prompting and project setup
- [Common workflows](https://code.claude.com/docs/en/common-workflows): step-by-step guides for debugging, testing, refactoring, and more
- [Terminal configuration](https://code.claude.com/docs/en/terminal-config): customize your terminal for the best Claude Code experience


---


## [​](https://code.claude.com/docs/en/terminal-guide#troubleshooting) Troubleshooting


### [​](https://code.claude.com/docs/en/terminal-guide#macos-and-linux-troubleshooting) macOS and Linux troubleshooting


If you run into problems installing on macOS or Linux, check these common issues:


'command not found: claude'

If you see `command not found: claude` after installing, your terminal needs to reload its settings. Close the Terminal window and open a new one, then try `claude` again. If it still doesn’t work, add the install directory to your PATH. Run the command for your shell:

```
# Zsh (macOS default)
echo 'export PATH="$HOME/.local/bin:$PATH"' >> ~/.zshrc
source ~/.zshrc

# Bash (Linux default)
echo 'export PATH="$HOME/.local/bin:$PATH"' >> ~/.bashrc
source ~/.bashrc
```

Then try `claude` again. For more details, see [fix your PATH](https://code.claude.com/docs/en/troubleshooting#verify-your-path).


Error with HTML code or 'syntax error near unexpected token'

If you see `bash: line 1: syntax error near unexpected token '<'` or HTML code like `<!DOCTYPE html>` in your terminal, the install URL returned a web page instead of the installer script. If the page says “App unavailable in region,” Claude Code is not available in your country. See [supported countries](https://www.anthropic.com/supported-countries). Otherwise, try running the command again. If it keeps happening, install with [Homebrew](https://brew.sh) instead:

```
brew install --cask claude-code
```


For other errors, see the full [installation troubleshooting guide](https://code.claude.com/docs/en/troubleshooting#troubleshoot-installation-issues).


### [​](https://code.claude.com/docs/en/terminal-guide#windows-troubleshooting) Windows troubleshooting


If you run into problems installing on Windows, check these common issues:


'irm is not recognized'

You’re in CMD, not PowerShell. Close this window and open PowerShell instead ( `Win + X` then select Windows PowerShell). Alternatively, use the CMD install command:

```
curl -fsSL https://claude.ai/install.cmd -o install.cmd && install.cmd && del install.cmd
```


SSL/TLS error or 'Could not create SSL/TLS secure channel'

This usually happens on older Windows 10 systems. Run this line first, then retry the install:

```
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
irm https://claude.ai/install.ps1 | iex
```


'Claude Code on Windows requires git-bash'

Git for Windows isn’t installed or Claude Code can’t find it.

1. If you haven’t installed Git yet, go back to the [first step in the Windows section](https://code.claude.com/docs/en/terminal-guide#windows).
2. If Git is installed but Claude Code can’t find it, tell it where to look: ``` $env:CLAUDE_CODE_GIT_BASH_PATH="C:\Program Files\Git\bin\bash.exe" ``` Then run `claude` again. If your Git is installed somewhere else, find the path by running: ``` Get-Command git | Select-Object Source ``` Look for the `Git\bin` folder in that path and use it instead.

To make this permanent so you don’t have to set it every time, see [configure Git Bash path](https://code.claude.com/docs/en/troubleshooting#windows-claude-code-on-windows-requires-git-bash).


'claude is not recognized'

Restart your computer and try again. This usually fixes the problem. If restarting didn’t help, run these commands to add Claude Code to your PATH:

```
$currentPath = [Environment]::GetEnvironmentVariable('PATH', 'User')
[Environment]::SetEnvironmentVariable('PATH', "$currentPath;$env:USERPROFILE\.local\bin", 'User')
```

Close PowerShell, open a new window, and try `claude` again. See [verify your PATH](https://code.claude.com/docs/en/troubleshooting#verify-your-path) for more details.
For other errors, see the full [installation troubleshooting guide](https://code.claude.com/docs/en/troubleshooting#troubleshoot-installation-issues).[Claude Code Docs home page](https://code.claude.com/docs/en/overview)

[Privacy choices](https://code.claude.com/docs/en/terminal-guide#)

