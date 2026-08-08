# 變更日誌

語言: [English](../CHANGELOG.md) | [日本語](./CHANGELOG.ja.md) | [簡體中文](./CHANGELOG.zh-CN.md) | 繁體中文

本檔案為英文主版 [`../CHANGELOG.md`](../CHANGELOG.md) 的繁體中文閱讀版本，提供參考。  
涉及發佈的權威版本資訊仍以頂層 `CHANGELOG.md` 為準；GitHub Release 的固定三語正文使用
English、日本語與簡體中文，本檔案不作為發佈正文輸入。

本檔案依循 [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) 約定，並遵守 [Semantic Versioning](https://semver.org/spec/v2.0.0.html)。
穩定版條目描述距離上一個穩定版的淨變更，預發布條目描述自上個已發佈版本（穩定或預發布）
以來的變更。

日常開發時，所有用戶可見變更請在同一 PR/提交中同步寫入主 `../CHANGELOG.md` 的 `Unreleased`。以下為簡體中文對照的繁體中文鏡像內容。

## [Unreleased]

### Added

- 新增透過共享資源管理後端進行 MCP 專案模板發現與管理。
- 新增將標準化變更日誌作為版本間的重要變更權威記錄。

### Changed

- 精簡 MCP 倉庫與模板資料欄位，並使用倉庫 URL 作為管理識別。
- 簡化 Unity 啟動狀態標籤。
- 將 GitHub Release 說明統一匯入此變更日誌，並將本地化 updater 摘要移到結構化的發佈 metadata。

### Fixed

- 在 Windows 保存環境設定時保留用戶自定義的套件路徑。
- 當有對應的 Unity 行程存在時，自動定位並聚焦到對應 Unity 編輯器。
- 本地化套件操作錯誤訊息，避免過長倉庫欄位溢出界面。

### Removed

- 刪除獨立的版本化發佈說明資料夾及其重複內容。

## [3.1.0] - 2026-08-01

### Added

- 新增 Bearer Token 保護的僅回環 MCP Streamable HTTP 端點，以及可選的客戶端初始化設定。
- 新增 Unity 專案開啟與就緒狀態追蹤、重複啟動防護與編輯器聚焦功能。
- 新增 MCP 的 VPM 依賴與 UnityPackage 關聯引用模板編輯支援。

### Changed

- 即使側邊欄項目被隱藏也保留擴充頁面可見性，將「專案 / 資源 / 設定」固定保留為側邊欄項目，修正既有組態導致的不可見問題。
- 允許停用內建 MCP 擴充：撤回存取、停止端點、移除側邊欄項目，並取消應用內 MCP 專案任務。
- 在 MCP 後台擴充及端點啟動繼續進行時，仍可顯示主視窗與 MCP 工具。
- 在刷新期間保持專案列表顯示，並及時更新專案類型與已儲存倉庫名稱。
- 在可選 MCP 設定時保留與舊 stdio 傳輸無關的客戶端設定，並要求相關客戶端切換到受保護端點與 Token。

### Security

- MCP 預設為停用，並在僅回環端點上驗證 Bearer 認證、主機與來源。

## [3.1.0-beta.3] - 2026-08-01

### Added

- 新增 Unity 專案開啟與就緒狀態追蹤、重複啟動防護與編輯器聚焦。

### Changed

- 在刷新過程保持專案列表可見、更新已儲存倉庫名稱，並更清晰呈現內建擴充行為。

## [3.1.0-beta.2] - 2026-07-28

### Added

- 允許停用內建 MCP 擴充：撤回存取、停止端點、移除側邊欄項目，並取消應用內 MCP 專案任務。

### Changed

- 在端點啟動於背景進行時仍顯示主視窗與 MCP 工具。
- 專案建立或 VRChat SDK 套件變更後，立即刷新專案類型。

## [3.1.0-beta.1] - 2026-07-28

### Added

- 新增支援 Bearer Token 的 MCP Streamable HTTP 端點，並為 Codex、Claude Code、Cursor 提供可選客戶端設定；同時保留非相關客戶端項目。

### Changed

- 在 MCP 設定對話框整合端點資訊與客戶端設定。
- 專案建立或 VRChat SDK 套件變更後即時刷新專案類型。
- 要求使用舊 stdio 傳輸的 MCP 客戶端切換至受保護端點並使用 Token。

### Security

- MCP 預設為停用，並在僅回環端點驗證 Bearer 認證、主機與來源。

## [3.0.1-beta.1] - 2026-07-27

### Added

- 為可見的已安裝擴充新增「開啟」按鈕。

### Changed

- 保持專案/資源/設定在側邊欄可見並修正持久條目顯示，恢復既有配置下的可視項。

## [3.0.0] - 2026-07-26

### Added

- 發佈第一個公開版本 ALCOMD3，支援 Windows x64、macOS Apple Silicon 與 Linux x86_64。

### Security

- 使用 ALCOMD3 專屬更新端點、內建公鑰與簽名更新負載。

[Unreleased]: https://github.com/ALCOMD3/ALCOMD3/compare/v3.1.0...HEAD
[3.1.0]: https://github.com/ALCOMD3/ALCOMD3/compare/v3.0.0...v3.1.0
[3.1.0-beta.3]: https://github.com/ALCOMD3/ALCOMD3/compare/v3.1.0-beta.2...v3.1.0-beta.3
[3.1.0-beta.2]: https://github.com/ALCOMD3/ALCOMD3/compare/v3.1.0-beta.1...v3.1.0-beta.2
[3.1.0-beta.1]: https://github.com/ALCOMD3/ALCOMD3/compare/v3.0.1-beta.1...v3.1.0-beta.1
[3.0.1-beta.1]: https://github.com/ALCOMD3/ALCOMD3/compare/v3.0.0...v3.0.1-beta.1
[3.0.0]: https://github.com/ALCOMD3/ALCOMD3/releases/tag/v3.0.0
