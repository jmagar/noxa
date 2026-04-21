# 託管 Agent SDK
## ​託管要求
## ​理解 SDK 架構
## ​沙箱提供商選項
## ​生產部署模式
## ​常見問題
## ​後續步驟







在生產環境中部署和託管 Claude Agent SDK

Claude Agent SDK 與傳統的無狀態 LLM API 不同，它維護對話狀態並在持久環境中執行命令。本指南涵蓋了在生產環境中部署基於 SDK 的代理的架構、託管考慮因素和最佳實踐。
如需超越基本沙箱的安全強化（包括網路控制、認證管理和隔離選項），請參閱 [安全部署](https://code.claude.com/docs/zh-TW/agent-sdk/secure-deployment)。


## [​](https://code.claude.com/docs/zh-TW/agent-sdk/hosting#%E8%A8%97%E7%AE%A1%E8%A6%81%E6%B1%82) 託管要求


### [​](https://code.claude.com/docs/zh-TW/agent-sdk/hosting#%E5%9F%BA%E6%96%BC%E5%AE%B9%E5%99%A8%E7%9A%84%E6%B2%99%E7%AE%B1) 基於容器的沙箱


為了安全和隔離，SDK 應在沙箱容器環境中運行。這提供了進程隔離、資源限制、網路控制和臨時文件系統。
SDK 還支持 [程序化沙箱配置](https://code.claude.com/docs/zh-TW/agent-sdk/typescript#sandbox-settings) 用於命令執行。


### [​](https://code.claude.com/docs/zh-TW/agent-sdk/hosting#%E7%B3%BB%E7%B5%B1%E8%A6%81%E6%B1%82) 系統要求


每個 SDK 實例需要：


- **運行時依賴項**
  - Python 3.10+ 用於 Python SDK，或 Node.js 18+ 用於 TypeScript SDK
  - 兩個 SDK 套件都為主機平台捆綁了本機 Claude Code 二進制文件，因此不需要為生成的 CLI 單獨安裝 Claude Code 或 Node.js
- **資源分配**
  - 建議：1GiB RAM、5GiB 磁盤和 1 個 CPU（根據您的任務需要進行調整）
- **網路訪問**
  - 出站 HTTPS 到 `api.anthropic.com`
  - 可選：訪問 MCP 伺服器或外部工具


## [​](https://code.claude.com/docs/zh-TW/agent-sdk/hosting#%E7%90%86%E8%A7%A3-sdk-%E6%9E%B6%E6%A7%8B) 理解 SDK 架構


與無狀態 API 調用不同，Claude Agent SDK 作為 **長時間運行的進程** 運行，該進程：


- **在持久 shell 環境中執行命令**
- **在工作目錄內管理文件操作**
- **使用來自先前交互的上下文處理工具執行**


## [​](https://code.claude.com/docs/zh-TW/agent-sdk/hosting#%E6%B2%99%E7%AE%B1%E6%8F%90%E4%BE%9B%E5%95%86%E9%81%B8%E9%A0%85) 沙箱提供商選項


多個提供商專門提供用於 AI 代碼執行的安全容器環境：


- **[Modal Sandbox](https://modal.com/docs/guide/sandbox)** - [演示實現](https://modal.com/docs/examples/claude-slack-gif-creator)
- **[Cloudflare Sandboxes](https://github.com/cloudflare/sandbox-sdk)**
- **[Daytona](https://www.daytona.io/)**
- **[E2B](https://e2b.dev/)**
- **[Fly Machines](https://fly.io/docs/machines/)**
- **[Vercel Sandbox](https://vercel.com/docs/functions/sandbox)**


有關自託管選項（Docker、gVisor、Firecracker）和詳細隔離配置，請參閱 [隔離技術](https://code.claude.com/docs/zh-TW/agent-sdk/secure-deployment#isolation-technologies)。


## [​](https://code.claude.com/docs/zh-TW/agent-sdk/hosting#%E7%94%9F%E7%94%A2%E9%83%A8%E7%BD%B2%E6%A8%A1%E5%BC%8F) 生產部署模式


### [​](https://code.claude.com/docs/zh-TW/agent-sdk/hosting#%E6%A8%A1%E5%BC%8F-1%EF%BC%9A%E8%87%A8%E6%99%82%E6%9C%83%E8%A9%B1) 模式 1：臨時會話


為每個用戶任務創建一個新容器，然後在完成時銷毀它。
最適合一次性任務，用戶可能仍然在任務完成時與 AI 交互，但一旦完成，容器就會被銷毀。
**示例：**


- 錯誤調查和修復：使用相關上下文調試和解決特定問題
- 發票處理：從收據/發票中提取和結構化數據用於會計系統
- 翻譯任務：在語言之間翻譯文檔或內容批次
- 圖像/視頻處理：對媒體文件應用轉換、優化或提取元數據


### [​](https://code.claude.com/docs/zh-TW/agent-sdk/hosting#%E6%A8%A1%E5%BC%8F-2%EF%BC%9A%E9%95%B7%E6%99%82%E9%96%93%E9%81%8B%E8%A1%8C%E7%9A%84%E6%9C%83%E8%A9%B1) 模式 2：長時間運行的會話


為長時間運行的任務維護持久容器實例。通常在容器內根據需求運行 **多個** Claude Agent 進程。
最適合主動代理，它們在沒有用戶輸入的情況下採取行動、提供內容的代理或處理大量消息的代理。
**示例：**


- 電子郵件代理：監控傳入電子郵件並根據內容自主進行分類、回應或採取行動
- 網站構建器：為每個用戶託管自定義網站，具有通過容器端口提供的實時編輯功能
- 高頻聊天機器人：處理來自 Slack 等平台的連續消息流，其中快速響應時間至關重要


### [​](https://code.claude.com/docs/zh-TW/agent-sdk/hosting#%E6%A8%A1%E5%BC%8F-3%EF%BC%9A%E6%B7%B7%E5%90%88%E6%9C%83%E8%A9%B1) 模式 3：混合會話


臨時容器，使用歷史和狀態進行補充，可能來自數據庫或 SDK 的會話恢復功能。
最適合與用戶進行間歇性交互的容器，啟動工作並在工作完成時關閉，但可以繼續。
**示例：**


- 個人項目經理：幫助管理進行中的項目，進行間歇性檢查，維護任務、決策和進度的上下文
- 深度研究：進行多小時的研究任務，保存發現並在用戶返回時恢復調查
- 客戶支持代理：處理跨越多個交互的支持票證，加載票證歷史和客戶上下文


### [​](https://code.claude.com/docs/zh-TW/agent-sdk/hosting#%E6%A8%A1%E5%BC%8F-4%EF%BC%9A%E5%96%AE%E5%80%8B%E5%AE%B9%E5%99%A8) 模式 4：單個容器


在一個全局容器中運行多個 Claude Agent SDK 進程。
最適合必須密切協作的代理。這可能是最不受歡迎的模式，因為您必須防止代理相互覆蓋。
**示例：**


- **模擬**：在模擬（如視頻遊戲）中相互交互的代理。


## [​](https://code.claude.com/docs/zh-TW/agent-sdk/hosting#%E5%B8%B8%E8%A6%8B%E5%95%8F%E9%A1%8C) 常見問題


### [​](https://code.claude.com/docs/zh-TW/agent-sdk/hosting#%E6%88%91%E5%A6%82%E4%BD%95%E8%88%87%E6%88%91%E7%9A%84%E6%B2%99%E7%AE%B1%E9%80%9A%E4%BF%A1%EF%BC%9F) 我如何與我的沙箱通信？


在容器中託管時，公開端口以與您的 SDK 實例通信。您的應用程序可以為外部客戶端公開 HTTP/WebSocket 端點，而 SDK 在容器內部運行。


### [​](https://code.claude.com/docs/zh-TW/agent-sdk/hosting#%E8%A8%97%E7%AE%A1%E5%AE%B9%E5%99%A8%E7%9A%84%E6%88%90%E6%9C%AC%E6%98%AF%E5%A4%9A%E5%B0%91%EF%BC%9F) 託管容器的成本是多少？


提供代理的主要成本是令牌；容器根據您配置的內容而異，但最低成本大約是每小時 5 美分。


### [​](https://code.claude.com/docs/zh-TW/agent-sdk/hosting#%E6%88%91%E6%87%89%E8%A9%B2%E4%BD%95%E6%99%82%E9%97%9C%E9%96%89%E7%A9%BA%E9%96%92%E5%AE%B9%E5%99%A8%E8%88%87%E4%BF%9D%E6%8C%81%E5%AE%83%E5%80%91%E6%BA%AB%E6%9A%96%EF%BC%9F) 我應該何時關閉空閒容器與保持它們溫暖？


這可能取決於提供商，不同的沙箱提供商將允許您為空閒超時設置不同的條件，之後沙箱可能會關閉。
您需要根據您認為用戶響應可能的頻率來調整此超時。


### [​](https://code.claude.com/docs/zh-TW/agent-sdk/hosting#%E6%88%91%E6%87%89%E8%A9%B2%E5%A4%9A%E4%B9%85%E6%9B%B4%E6%96%B0%E4%B8%80%E6%AC%A1-claude-code-cli%EF%BC%9F) 我應該多久更新一次 Claude Code CLI？


Claude Code CLI 使用 semver 進行版本控制，因此任何破壞性更改都將被版本化。


### [​](https://code.claude.com/docs/zh-TW/agent-sdk/hosting#%E6%88%91%E5%A6%82%E4%BD%95%E7%9B%A3%E6%8E%A7%E5%AE%B9%E5%99%A8%E5%81%A5%E5%BA%B7%E5%92%8C%E4%BB%A3%E7%90%86%E6%80%A7%E8%83%BD%EF%BC%9F) 我如何監控容器健康和代理性能？


由於容器只是伺服器，您用於後端的相同日誌記錄基礎設施將適用於容器。


### [​](https://code.claude.com/docs/zh-TW/agent-sdk/hosting#%E4%BB%A3%E7%90%86%E6%9C%83%E8%A9%B1%E5%9C%A8%E8%B6%85%E6%99%82%E5%89%8D%E5%8F%AF%E4%BB%A5%E9%81%8B%E8%A1%8C%E5%A4%9A%E9%95%B7%E6%99%82%E9%96%93%EF%BC%9F) 代理會話在超時前可以運行多長時間？


代理會話不會超時，但考慮設置 ‘maxTurns’ 屬性以防止 Claude 陷入循環。


## [​](https://code.claude.com/docs/zh-TW/agent-sdk/hosting#%E5%BE%8C%E7%BA%8C%E6%AD%A5%E9%A9%9F) 後續步驟


- [安全部署](https://code.claude.com/docs/zh-TW/agent-sdk/secure-deployment) - 網路控制、認證管理和隔離強化
- [TypeScript SDK - Sandbox Settings](https://code.claude.com/docs/zh-TW/agent-sdk/typescript#sandbox-settings) - 以程序方式配置沙箱
- [會話指南](https://code.claude.com/docs/zh-TW/agent-sdk/sessions) - 了解會話管理
- [權限](https://code.claude.com/docs/zh-TW/agent-sdk/permissions) - 配置工具權限
- [成本追蹤](https://code.claude.com/docs/zh-TW/agent-sdk/cost-tracking) - 監控 API 使用情況
- [MCP 集成](https://code.claude.com/docs/zh-TW/agent-sdk/mcp) - 使用自定義工具進行擴展[Claude Code Docs home page](https://code.claude.com/docs/zh-TW/overview)

[Privacy choices](https://code.claude.com/docs/zh-TW/agent-sdk/hosting#)

