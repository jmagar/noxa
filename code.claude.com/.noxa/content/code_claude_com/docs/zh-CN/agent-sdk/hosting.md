# 托管 Agent SDK
## ​托管要求
## ​理解 SDK 架构
## ​Sandbox 提供商选项
## ​生产部署模式
## ​常见问题
## ​后续步骤







在生产环境中部署和托管 Claude Agent SDK

Claude Agent SDK 与传统的无状态 LLM API 不同，它维护对话状态并在持久环境中执行命令。本指南涵盖了在生产环境中部署基于 SDK 的代理的架构、托管考虑因素和最佳实践。
有关超越基本 sandboxing 的安全加固（包括网络控制、凭证管理和隔离选项），请参阅 [Secure Deployment](https://code.claude.com/docs/zh-CN/agent-sdk/secure-deployment)。


## [​](https://code.claude.com/docs/zh-CN/agent-sdk/hosting#%E6%89%98%E7%AE%A1%E8%A6%81%E6%B1%82) 托管要求


### [​](https://code.claude.com/docs/zh-CN/agent-sdk/hosting#%E5%9F%BA%E4%BA%8E%E5%AE%B9%E5%99%A8%E7%9A%84-sandboxing) 基于容器的 Sandboxing


为了安全性和隔离，SDK 应在沙箱容器环境中运行。这提供了进程隔离、资源限制、网络控制和临时文件系统。
SDK 还支持 [programmatic sandbox configuration](https://code.claude.com/docs/zh-CN/agent-sdk/typescript#sandbox-settings) 用于命令执行。


### [​](https://code.claude.com/docs/zh-CN/agent-sdk/hosting#%E7%B3%BB%E7%BB%9F%E8%A6%81%E6%B1%82) 系统要求


每个 SDK 实例需要：


- **运行时依赖**
  - Python 3.10+ 用于 Python SDK，或 Node.js 18+ 用于 TypeScript SDK
  - 两个 SDK 包都为主机平台捆绑了本地 Claude Code 二进制文件，因此不需要为生成的 CLI 单独安装 Claude Code 或 Node.js
- **资源分配**
  - 推荐：1GiB RAM、5GiB 磁盘和 1 个 CPU（根据您的任务需要调整）
- **网络访问**
  - 出站 HTTPS 到 `api.anthropic.com`
  - 可选：访问 MCP 服务器或外部工具


## [​](https://code.claude.com/docs/zh-CN/agent-sdk/hosting#%E7%90%86%E8%A7%A3-sdk-%E6%9E%B6%E6%9E%84) 理解 SDK 架构


与无状态 API 调用不同，Claude Agent SDK 作为 **长运行进程** 运行，该进程：


- **在持久 shell 环境中执行命令**
- **在工作目录中管理文件操作**
- **处理工具执行**，包含来自先前交互的上下文


## [​](https://code.claude.com/docs/zh-CN/agent-sdk/hosting#sandbox-%E6%8F%90%E4%BE%9B%E5%95%86%E9%80%89%E9%A1%B9) Sandbox 提供商选项


几个提供商专门提供用于 AI 代码执行的安全容器环境：


- **[Modal Sandbox](https://modal.com/docs/guide/sandbox)** - [demo implementation](https://modal.com/docs/examples/claude-slack-gif-creator)
- **[Cloudflare Sandboxes](https://github.com/cloudflare/sandbox-sdk)**
- **[Daytona](https://www.daytona.io/)**
- **[E2B](https://e2b.dev/)**
- **[Fly Machines](https://fly.io/docs/machines/)**
- **[Vercel Sandbox](https://vercel.com/docs/functions/sandbox)**


有关自托管选项（Docker、gVisor、Firecracker）和详细的隔离配置，请参阅 [Isolation Technologies](https://code.claude.com/docs/zh-CN/agent-sdk/secure-deployment#isolation-technologies)。


## [​](https://code.claude.com/docs/zh-CN/agent-sdk/hosting#%E7%94%9F%E4%BA%A7%E9%83%A8%E7%BD%B2%E6%A8%A1%E5%BC%8F) 生产部署模式


### [​](https://code.claude.com/docs/zh-CN/agent-sdk/hosting#%E6%A8%A1%E5%BC%8F-1%EF%BC%9A%E4%B8%B4%E6%97%B6%E4%BC%9A%E8%AF%9D) 模式 1：临时会话


为每个用户任务创建一个新容器，然后在完成时销毁它。
最适合一次性任务，用户可能在任务完成时仍与 AI 交互，但一旦完成，容器就会被销毁。
**示例：**


- Bug 调查和修复：使用相关上下文调试和解决特定问题
- 发票处理：从收据/发票中提取和结构化数据用于会计系统
- 翻译任务：在语言之间翻译文档或内容批次
- 图像/视频处理：对媒体文件应用转换、优化或提取元数据


### [​](https://code.claude.com/docs/zh-CN/agent-sdk/hosting#%E6%A8%A1%E5%BC%8F-2%EF%BC%9A%E9%95%BF%E8%BF%90%E8%A1%8C%E4%BC%9A%E8%AF%9D) 模式 2：长运行会话


为长运行任务维护持久容器实例。通常在容器内根据需求运行 **多个** Claude Agent 进程。
最适合主动代理，这些代理在没有用户输入的情况下采取行动，提供内容的代理或处理大量消息的代理。
**示例：**


- 电子邮件代理：监控传入电子邮件并根据内容自主分类、响应或采取行动
- 网站构建器：为每个用户托管自定义网站，具有通过容器端口提供的实时编辑功能
- 高频聊天机器人：处理来自 Slack 等平台的连续消息流，其中需要快速响应时间


### [​](https://code.claude.com/docs/zh-CN/agent-sdk/hosting#%E6%A8%A1%E5%BC%8F-3%EF%BC%9A%E6%B7%B7%E5%90%88%E4%BC%9A%E8%AF%9D) 模式 3：混合会话


临时容器，使用历史和状态进行补充，可能来自数据库或 SDK 的会话恢复功能。
最适合与用户进行间歇性交互的容器，启动工作并在工作完成时关闭，但可以继续。
**示例：**


- 个人项目管理器：帮助管理进行中的项目，进行间歇性检查，维护任务、决策和进度的上下文
- 深度研究：进行多小时的研究任务，保存发现并在用户返回时恢复调查
- 客户支持代理：处理跨越多个交互的支持票证，加载票证历史和客户上下文


### [​](https://code.claude.com/docs/zh-CN/agent-sdk/hosting#%E6%A8%A1%E5%BC%8F-4%EF%BC%9A%E5%8D%95%E4%B8%AA%E5%AE%B9%E5%99%A8) 模式 4：单个容器


在一个全局容器中运行多个 Claude Agent SDK 进程。
最适合必须紧密协作的代理。这可能是最不受欢迎的模式，因为您必须防止代理相互覆盖。
**示例：**


- **模拟**：在模拟中相互交互的代理，例如视频游戏。


## [​](https://code.claude.com/docs/zh-CN/agent-sdk/hosting#%E5%B8%B8%E8%A7%81%E9%97%AE%E9%A2%98) 常见问题


### [​](https://code.claude.com/docs/zh-CN/agent-sdk/hosting#%E6%88%91%E5%A6%82%E4%BD%95%E4%B8%8E%E6%88%91%E7%9A%84-sandboxes-%E9%80%9A%E4%BF%A1%EF%BC%9F) 我如何与我的 sandboxes 通信？


在容器中托管时，暴露端口以与您的 SDK 实例通信。您的应用程序可以为外部客户端暴露 HTTP/WebSocket 端点，而 SDK 在容器内部运行。


### [​](https://code.claude.com/docs/zh-CN/agent-sdk/hosting#%E6%89%98%E7%AE%A1%E5%AE%B9%E5%99%A8%E7%9A%84%E6%88%90%E6%9C%AC%E6%98%AF%E5%A4%9A%E5%B0%91%EF%BC%9F) 托管容器的成本是多少？


提供代理的主要成本是令牌；容器根据您配置的内容而异，但最低成本大约是每小时运行 5 美分。


### [​](https://code.claude.com/docs/zh-CN/agent-sdk/hosting#%E6%88%91%E5%BA%94%E8%AF%A5%E4%BD%95%E6%97%B6%E5%85%B3%E9%97%AD%E7%A9%BA%E9%97%B2%E5%AE%B9%E5%99%A8%E4%B8%8E%E4%BF%9D%E6%8C%81%E5%AE%83%E4%BB%AC%E6%B8%A9%E6%9A%96%EF%BC%9F) 我应该何时关闭空闲容器与保持它们温暖？


这可能取决于提供商，不同的 sandbox 提供商将让您为空闲超时设置不同的条件，之后 sandbox 可能会关闭。
您需要根据您认为用户响应可能的频率来调整此超时。


### [​](https://code.claude.com/docs/zh-CN/agent-sdk/hosting#%E6%88%91%E5%BA%94%E8%AF%A5%E5%A4%9A%E4%B9%85%E6%9B%B4%E6%96%B0%E4%B8%80%E6%AC%A1-claude-code-cli%EF%BC%9F) 我应该多久更新一次 Claude Code CLI？


Claude Code CLI 使用 semver 进行版本控制，因此任何破坏性更改都将被版本化。


### [​](https://code.claude.com/docs/zh-CN/agent-sdk/hosting#%E6%88%91%E5%A6%82%E4%BD%95%E7%9B%91%E6%8E%A7%E5%AE%B9%E5%99%A8%E5%81%A5%E5%BA%B7%E5%92%8C%E4%BB%A3%E7%90%86%E6%80%A7%E8%83%BD%EF%BC%9F) 我如何监控容器健康和代理性能？


由于容器只是服务器，您用于后端的相同日志记录基础设施将适用于容器。


### [​](https://code.claude.com/docs/zh-CN/agent-sdk/hosting#%E4%BB%A3%E7%90%86%E4%BC%9A%E8%AF%9D%E5%9C%A8%E8%B6%85%E6%97%B6%E5%89%8D%E5%8F%AF%E4%BB%A5%E8%BF%90%E8%A1%8C%E5%A4%9A%E9%95%BF%E6%97%B6%E9%97%B4%EF%BC%9F) 代理会话在超时前可以运行多长时间？


代理会话不会超时，但考虑设置 ‘maxTurns’ 属性以防止 Claude 陷入循环。


## [​](https://code.claude.com/docs/zh-CN/agent-sdk/hosting#%E5%90%8E%E7%BB%AD%E6%AD%A5%E9%AA%A4) 后续步骤


- [Secure Deployment](https://code.claude.com/docs/zh-CN/agent-sdk/secure-deployment) - 网络控制、凭证管理和隔离加固
- [TypeScript SDK - Sandbox Settings](https://code.claude.com/docs/zh-CN/agent-sdk/typescript#sandbox-settings) - 以编程方式配置 sandbox
- [Sessions Guide](https://code.claude.com/docs/zh-CN/agent-sdk/sessions) - 了解会话管理
- [Permissions](https://code.claude.com/docs/zh-CN/agent-sdk/permissions) - 配置工具权限
- [Cost Tracking](https://code.claude.com/docs/zh-CN/agent-sdk/cost-tracking) - 监控 API 使用情况
- [MCP Integration](https://code.claude.com/docs/zh-CN/agent-sdk/mcp) - 使用自定义工具扩展[Claude Code Docs home page](https://code.claude.com/docs/zh-CN/overview)

[Privacy choices](https://code.claude.com/docs/zh-CN/agent-sdk/hosting#)

