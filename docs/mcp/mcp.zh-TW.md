# ALCOMD3 MCP 說明

語言: [English](../mcp.md) | [日本語](mcp.ja.md) | [簡體中文](mcp.zh-CN.md) | 繁體中文

本文件說明 ALCOMD3 的 MCP 接入方式、可用工具、生命週期行為和疑難排解方法。

ALCOMD3 使用 RMCP 3.1.2 實作 MCP `2026-07-28`，並相容 `2025-11-25`
用戶端的一般工具呼叫。MCP server 已成為 GUI 行程的一部分：MCP 擴充功能啟用時，GUI
以 `alcomd3-mcp` implementation 名稱在 `127.0.0.1` 暴露一個本機 Streamable HTTP
endpoint，不再有 helper 行程、私有 IPC listener 或 endpoint metadata 檔案。

## 快速開始

1. 啟動 ALCOMD3，在「擴充功能」頁確認 MCP 擴充功能已啟用，再開啟側邊欄中的 MCP 頁面。
2. 啟用 MCP。
3. 預設使用頁面顯示的 MCP Endpoint 和授權權杖手動設定用戶端。Windows 上的 Codex、
   Claude Code 和 Cursor 使用者也可以選擇對應的快速設定按鈕。
4. 手動設定時，將 URL 新增為 Streamable HTTP server，並用
   `Authorization: Bearer <權杖>` 傳送授權權杖。
5. 保持 ALCOMD3 中的 MCP 為啟用狀態，然後執行工具呼叫。

請直接使用 GUI 顯示的 endpoint，不要自行猜測連接埠。設定範例和生命週期詳情請參閱
[啟用與客戶端設定](#啟用與客戶端設定)。

## 目前邊界

- MCP 功能預設停用，需要在 GUI 中手動啟用後才允許新的工具呼叫讀取或寫入 ALCOMD3 資料。
- MCP 擴充功能啟用時，GUI 會執行本機 Streamable HTTP endpoint；在 MCP 頁面
  啟用/停用 MCP 只控制新的工具資料存取，不會關閉 endpoint。
- 從「擴充功能」頁關閉 MCP 擴充功能會撤銷 MCP 存取權、停止 endpoint、從側邊欄
  移除 MCP，並取消仍由 GUI 管理的 MCP 專案任務。重新啟用擴充功能會立即完成開關
  操作，並在背景恢復 endpoint；MCP 存取仍保持停用，直到使用者再次在 MCP 頁面主動啟用。
- 目前提供專案、環境層級範本、倉庫、軟體包和環境設定唯讀工具，以及有限寫工具：新建專案、
  建立/編輯/刪除環境層級範本、為衍生範本單獨設定或移除一項直接 VPM 相依或 UnityPackage
  附件引用、新增既有專案、新增或刪除使用者 VPM 倉庫、備份已登錄專案、複製已登錄專案、從 zip 備份還原專案、
  為已登錄專案安裝/解除安裝/重新安裝單一軟體包。不提供倉庫重新排序、專案刪除等其他寫操作。
- MCP 擴充功能啟用時，GUI 負責啟動和管理本機 Streamable HTTP server；關閉 GUI 或
  關閉 MCP 擴充功能時都會停止該 server。
- 使用 MCP 工具期間必須保持 ALCOMD3 執行；關閉 GUI 也會關閉公開的 loopback endpoint。
- MCP 停用時，新的 tool call 傳回結構化 `mcp_disabled` 錯誤，不關閉 endpoint、不 panic，
  並在 MCP tool result 上標記 `isError: true`。已啟動專案長任務的 `tasks/get`、
  `tasks/cancel` 是收尾例外，可繼續查詢結果或取消該任務。
- 內建 server 對 tool call 做本機速率限制和並行保護；超過限制時傳回結構化
  `rate_limited` 錯誤，並在 MCP tool result 上標記 `isError: true`。
- GUI MCP 頁面會在已知 tool call 執行時醒目標示對應工具，並在完成或失敗後短暫保留醒目標示，
  便於觀察很快完成的呼叫。
- GUI MCP 頁面按唯讀、寫入和日誌用途分組顯示工具，並保留工具的精確 MCP 名稱；滑鼠懸停在工具名稱上會顯示在地化的可讀名稱。
- GUI MCP 頁面顯示的是最近活動過的客戶端，不是即時連線清單；超過一段時間沒有活動的記錄
  會自動隱藏。
- MCP tool call 會寫入 GUI 的本機活動記錄。記錄包含來源、工具名稱、request id、客戶端摘要、
  開始/完成/失敗/取消狀態和經過安全處理的目標/詳細資料，便於使用者在 GUI 的“活動記錄”頁回溯 Agent 做了什麼。
- GUI 專案管理頁和 MCP 包工具共用後端的 GUI-visible package catalog。預發布、yanked、
  隱藏倉庫、隱藏本機使用者包、同名包跨來源合併、預設/使用者倉庫優先順序和 Unity 相容性判斷由後端統一執行。
- 每個公開 MCP tool 都必須映射到 GUI 已有 capability，並透過 `vrc-get-gui/src/backend/`
  中的共享後端服務進入業務邏輯。MCP dispatch 只負責啟用狀態 gate、參數解析、任務封裝、
  錯誤映射和活動記錄，不應新增 GUI 不具備的業務能力。
- Streamable HTTP 請求必須攜帶為本機 ALCOMD3 安裝產生的 bearer token。
- HTTP server 會驗證 `Host` 和 `Origin`，嚴格綁定 `127.0.0.1`，不監聽區域網路或公網位址。

活動記錄不會儲存原始 MCP params、token-like 欄位、HTTP header 值、帶 query 的 URL 或 URL userinfo 憑證。
本機檔案系統路徑會保留完整值，用於排查 Unity、VPM 和中文路徑等問題；MCP access 仍需先在 GUI 中啟用。

## 架構

```text
MCP Host / Client
        |
        | Streamable HTTP + bearer token
        v
ALCOMD3 GUI process
        |
        +-- embedded alcomd3-mcp HTTP/RMCP service
        +-- direct in-process GUI business dispatch
        +-- shared Tasks, cancellation, locks, and activity state
```

所有已驗證請求都在 GUI 行程內處理。工具 handler 直接呼叫 GUI 使用的同一套後端服務，
不再經過 TCP bridge 或 JSON 行序列化。所有已驗證本機用戶端共享同一個 bearer principal
和任務命名空間；用戶端名稱與版本只用於最近活動顯示與日誌。

## 啟用與客戶端設定

1. 啟動 ALCOMD3。
2. 在「擴充功能」頁確認 MCP 擴充功能已啟用，再打開側邊欄中的 MCP 頁面。
3. 點擊啟用，允許 MCP 工具讀取 ALCOMD3 資料。
4. 複製頁面中的 MCP Endpoint 和授權權杖。
5. 在支援 Streamable HTTP MCP server 的客戶端中新增 endpoint URL，並將權杖設定為
   bearer `Authorization` header。

通用設定形態如下，具體欄位名稱以 MCP 客戶端為準：

```json
{
    "mcpServers": {
        "alcomd3": {
            "url": "http://127.0.0.1:51739/mcp",
            "headers": {
                "Authorization": "Bearer <ALCOMD3 頁面顯示的權杖>"
            }
        }
    }
}
```

手動設定仍是預設方式，不會修改作業系統或 AI 用戶端設定。

### Windows 上選用的 AI 用戶端快速設定

Windows 的 MCP 頁面為 Codex、Claude Code 和 Cursor 提供各自獨立的快速設定按鈕。
只有使用者明確點擊某個按鈕後，才會修改對應用戶端。每個按鈕都會將目前權杖寫入目前
Windows 使用者的 `ALCOMD3_MCP_BEARER_TOKEN` 環境變數，並僅新增或更新所選用戶端的
ALCOMD3 MCP 設定項目：

- Codex：使用 `$CODEX_HOME/config.toml`；未設定 `CODEX_HOME` 時使用
  `~/.codex/config.toml`：

```toml
[mcp_servers.alcomd3]
url = "http://127.0.0.1:51739/mcp"
bearer_token_env_var = "ALCOMD3_MCP_BEARER_TOKEN"
```

- Claude Code：使用 `$CLAUDE_CONFIG_DIR/.claude.json`；未設定
  `CLAUDE_CONFIG_DIR` 時使用 `~/.claude.json`：

```json
{
    "mcpServers": {
        "alcomd3": {
            "type": "http",
            "url": "http://127.0.0.1:51739/mcp",
            "headers": {
                "Authorization": "Bearer ${ALCOMD3_MCP_BEARER_TOKEN}"
            }
        }
    }
}
```

- Cursor：使用 `~/.cursor/mcp.json`：

```json
{
    "mcpServers": {
        "alcomd3": {
            "type": "http",
            "url": "http://127.0.0.1:51739/mcp",
            "headers": {
                "Authorization": "Bearer ${env:ALCOMD3_MCP_BEARER_TOKEN}"
            }
        }
    }
}
```

所選用戶端的其他設定和 MCP server 會原樣保留。如果環境變數或該用戶端的 `alcomd3`
設定項目已存在不同值，ALCOMD3 會先要求確認，不會靜默覆寫。完成快速設定後請完全結束
並重新啟動所選用戶端，使其繼承新的使用者環境變數並重新載入 MCP 設定。

不同客戶端的欄位名稱可能不同。請一律從 GUI 複製目前 URL 與權杖。預設連接埠為
`51739`；進階使用者可在啟動 ALCOMD3 前修改 `gui-config.json` 中的 `mcpHttpPort`。

endpoint 只在 ALCOMD3 執行且 MCP 擴充功能啟用時可用。如果 MCP 頁面的存取權已停用，
新的工具呼叫會傳回 `mcp_disabled`，啟用 MCP 後重試即可。關閉 MCP 擴充功能會停止
endpoint，並取消仍由 GUI 管理的 MCP 專案任務。

外部 HTTP 連接埠和 bearer token 以 `mcpHttpPort`、`mcpHttpToken` 儲存在
`gui-config.json`。請將 token 視為本機密鑰，不要放入日誌、截圖或共用設定。

## 內建執行階段與持久設定

所有受支援安裝包與封存檔都只透過 GUI 可執行檔提供 MCP，不再包含需要定位或啟動的
`alcomd3-mcp` helper。`cargo xtask build-alcom` 只建置 GUI，由 GUI 行程持有 HTTP
listener 並執行全部工具。

公開連接埠和 bearer token 以 `mcpHttpPort`、`mcpHttpToken` 儲存在
`gui-config.json`。執行時不再讀取、寫入或遷移 `mcp/endpoint.json`；
`ALCOMD3_MCP_ENDPOINT_FILE` 和內部 listener 覆寫項目已移除，用戶端設定仍可使用
`ALCOMD3_MCP_BEARER_TOKEN`。

修改連接埠或輪換權杖會依序停止並重新綁定內建 transport。transport 重新啟動期間，
共享的協定任務狀態仍保留在 GUI 中。新連接埠綁定失敗不會影響 GUI 其他功能；MCP
頁面會顯示 server 未執行，並由技術日誌記錄失敗。

## 可用工具

ALCOMD3 目前公開 33 個工具。主指南集中說明使用流程和安全邊界；
[完整工具參考](tools.zh-TW.md)逐項列出每個輸入、輸出欄位，說明是否必填或按條件出現、
省略時的預設值，以及欄位的實際含義。

| 領域 | 讀取工具 | 寫入工具 |
| --- | --- | --- |
| 專案 | 專案清單和詳細資料 | 建立、登錄、備份、複製和還原 |
| 範本 | 範本清單和詳細資料 | 建立、編輯、設定/移除 VPM 相依和 UnityPackage 引用，以及刪除 |
| 倉庫 | 倉庫清單 | 新增和刪除遠端使用者倉庫 |
| 軟體包 | 軟體包清單和詳細資料 | 安裝、解除安裝和重新安裝專案軟體包 |
| 環境 | Unity 安裝、啟動參數和預設路徑 | 無 |
| 記錄 | 搜尋、詳細資料、上下文和彙總 | 無 |

工具參考還記錄分頁預設值、允許的列舉、MCP Task 支援、共用傳回類型、錯誤結構，
以及兩個按設計直接傳回詳細資料物件而不含 `ok` 的詳細資料工具。

### 日誌查詢工具

日誌工具按用途分為“活動記錄”和“技術日誌”兩套，避免 Agent 為了排查一個問題把全部日誌拉入上下文。

- 活動記錄是使用者可讀、結構化、已脫敏的操作歷史。`alcomd3_search_activity_logs`
  預設 `visibility` 為 `important`，會傳回寫操作、失敗、取消和重要 MCP/System 行為等關鍵活動；
  需要輔助記錄時明確傳入 `secondary`、`technical` 或 `all`。
- 活動日誌搜尋結果只傳回摘要欄位，包括 id、時間、來源、類型、狀態、操作、物件、耗時和錯誤摘要。
  需要詳細資料時再呼叫 `alcomd3_get_activity_log_entry`，需要上下文時呼叫
  `alcomd3_get_activity_log_context`。
- 技術日誌是排錯入口，預設只查目前行程記憶體裡的 `error` 和 `warn`。需要讀取近期檔案時明確傳入
  `"scope": "recent_files"`；需要 Info/Debug/Trace 時明確傳入 `levels`。
- 技術日誌工具不會傳回無限制原文。搜尋只傳回 `messagePreview`，詳細資料按 `max_message_chars`
  截斷，並會脫敏 token、secret、authorization、API key、`sk-` 開頭的值，以及 URL userinfo、query 和 fragment。
- 日誌工具本身也會被記錄為 MCP read activity。成功讀取日誌屬於 Secondary，失敗仍會作為失敗活動預設可見。

### 專案長任務

ALCOMD3 使用 RMCP 3.1.2 提供的實驗性 `io.modelcontextprotocol/tasks` 擴充功能。
不同用戶端的支援程度可能不同，該擴充功能也可能在後續版本中演進。

`alcomd3_create_project`、`alcomd3_backup_project`、`alcomd3_copy_project`、
`alcomd3_restore_project_from_backup`、`alcomd3_install_project_package`、
`alcomd3_uninstall_project_package` 和 `alcomd3_reinstall_project_package` 會在用戶端宣告
`io.modelcontextprotocol/tasks` capability 時使用 task-aware 呼叫：

- `tools/call` 會立即傳回帶 `taskId` 的 task handle。
- `tasks/get` 傳回 `working`、`input_required`、`completed`、`failed` 或 `cancelled`，
  並在詳細任務狀態中包含完成結果或失敗資訊。
- `tasks/update` 向執行中的任務提供其請求的回應。
- `tasks/cancel` 會協作式取消底層 GUI 操作，並釋放對應資源鎖。
- `alcomd3_create_project` 在專案正式登錄前收到取消或包解析/套用失敗時，會清理 MCP 建立出的未登錄專案目錄。
- 如果任務執行期間使用者停用 MCP，新的工具呼叫和新的專案任務啟動仍會傳回
  `mcp_disabled`；已獲得 `taskId` 的任務仍可由已驗證請求使用 `tasks/get` 查詢或
  `tasks/cancel` 取消。
- 關閉整個 MCP 擴充功能或結束 GUI 會取消未完成任務並清理其協定狀態。

此擴充功能有意不提供舊 core Tasks 的 `tasks/list` 和 `tasks/result`；完成輸出直接從
`tasks/get` 讀取。同步 `tools/call` 帶 `_meta.progressToken` 時仍會收到標準
`notifications/progress`；task-aware 呼叫也會隨後端進度更新可讀狀態資訊。

未宣告 Tasks capability 的用戶端繼續取得原有一般同步 `tools/call` 行為和結果形狀。

### 路徑限制

`alcomd3_get_project_details`、`alcomd3_backup_project`、`alcomd3_copy_project`
以及專案包安裝/解除安裝/重新安裝工具的來源專案路徑只允許使用 ALCOMD3 資料庫中已登記的專案路徑。MCP client 不能透過這些工具
讀取或複製任意本機路徑。

`alcomd3_get_environment_settings` 會傳回 ALCOMD3 已儲存的本機路徑，例如 Unity 可執行檔、
預設專案目錄和備份目錄。該工具不啟動 Unity、不呼叫 Unity Hub 刷新、不掃描額外磁碟路徑。

`alcomd3_backup_project` 的 `backup_name` 只允許是單一合法檔名，不能是路徑，且不包含自動附加的
`.zip` 副檔名。封存檔始終寫入 GUI 設定的備份目錄，且不會覆寫現有封存檔。

`alcomd3_copy_project` 的 `new_project_path` 必須是絕對路徑、尚不存在的目錄路徑，且不能位於
來源專案目錄內部；工具會建立該目錄，複製專案檔案後登記新專案，失敗時會清理新建目錄。
`alcomd3_restore_project_from_backup` 的 `backup_path` 必須是絕對路徑，並且只從 zip 備份還原到
GUI 設定的預設專案目錄。`project_name` 只允許是單一合法資料夾名稱，不能包含路徑分隔符、
根路徑或 `..`。
`alcomd3_create_project` 的 `project_name` 使用同樣的單一資料夾名稱限制；明確傳入的 `base_path`
必須是絕對路徑。未傳 `base_path` 時使用 GUI 預設專案路徑。`alcomd3_add_existing_project`
的 `project_path` 必須是絕對路徑，並且必須能按 Unity 專案載入。

### 軟體包可見性與寫入限制

`alcomd3_list_packages` 和 `alcomd3_list_repository_packages` 使用與 GUI 軟體包頁相同的包狀態載入路徑，不呼叫強制重新整理路徑。
傳回結果會遵循 GUI 中的預發布、隱藏倉庫、隱藏本機使用者包和 yanked 篩選規則。MCP tool call
不做伺服器端搜尋。新增倉庫必須明確呼叫 `alcomd3_add_repository`；列表工具不會隱式新增倉庫或重構倉庫重新整理策略。

倉庫參數的用途彼此分離：軟體包讀取和安裝工具統一使用 `alcomd3_list_repositories` 傳回的 `id` 選擇倉庫；使用者倉庫的新增和刪除使用已存 URL，因此刪除輸入與新增輸入直接對應，並且不會作用於內建預設倉庫。重複檢查仍同時涵蓋已存 URL 和倉庫發佈者宣告的 ID。GUI 的新增、刪除和重新排序也使用同一個以 URL 為基礎的共享後端。不支援本機倉庫：載入設定時會捨棄無 URL 的使用者倉庫項目，也不提供本機倉庫建立路徑。

GUI 專案管理頁的軟體包表由後端合併同名包產生。MCP 的包列表、包詳細資料和專案包安裝選擇使用同一套後端規則：

- 關閉“顯示預發布軟體包”後，GUI 和 MCP 的 GUI-visible 結果都不會包含預發布版本；MCP `latest_gui_visible`
  也無法選擇預發布版本。底層快取仍可儲存預發布資料，重新開啟後才會進入可見結果。
- yanked 包不會進入可見候選。已安裝包如果目前版本 yanked，會在專案包行中保留 yanked 標記。
- 隱藏倉庫和隱藏本機使用者包只影響可見候選；隱藏來源仍可作為“存在來源”資訊顯示，但不參與最新版本選擇。
- 同名包跨來源在專案管理頁合併成一行，預設倉庫、本機使用者包、使用者倉庫和未登錄倉庫依後端順序合併。
- 專案包安裝只會從 GUI-visible 且與專案 Unity 版本相容的候選中選擇版本。

`alcomd3_install_project_package`、`alcomd3_uninstall_project_package` 和
`alcomd3_reinstall_project_package` 會先產生 pending project changes。若結果包含相依性衝突或 legacy
檔案/資料夾刪除，且未傳入 `"allow_conflicts": true`，工具會傳回
`project_package_conflicts`，並在 `error.data.changes` 中附帶變更摘要；此時不會套用到專案。
確認後重試並設定 `"allow_conflicts": true` 才會繼續 apply。

包列表工具只傳回適合發現和篩選的摘要欄位：`name`、`displayName`、`version` 和 `source`。
列表中的 `totalCount` 和分頁欄位按彙總後的摘要條目计算，不是倉庫原始版本清單的長度。
需要讀取描述、關鍵字、相依性、legacy 包、文檔 URL、變更日誌 URL 或 Unity 版本要求時，應先從列表中選出候選包，
再呼叫 `alcomd3_get_package_details` 取得詳細元資料。

包列表工具預設 `offset` 為 `0`、`limit` 為 `200`；`limit` 最大為 `1000`，超過時會被限制到最大值。
分頁回應包含 `totalCount`、`offset`、`limit`、`returnedCount`、`hasMore` 和 `nextOffset`。
需要讀取完整清單時，應在 `hasMore` 為 `true` 時使用 `nextOffset` 繼續請求下一頁。
包相關工具不再傳回 `count` 欄位。

## 生命週期和用戶端行為

GUI 載入本機設定後會綁定一個內建 `alcomd3-mcp` Streamable HTTP server。
MCP `2026-07-28` 請求沒有 session，而且每次請求都必須攜帶標準協定 metadata；一般
`2025-11-25` 用戶端繼續使用 legacy session。兩條路徑共享 GUI 狀態、速率限制器、
任務管理器、資源鎖和活動記錄器。

ALCOMD3 的生命週期邊界：

- GUI 結束或 MCP 擴充功能關閉時會停止 HTTP listener，等待服務任務結束並取消未完成操作。
- endpoint URL 與 bearer token 對目前本機安裝保持穩定；GUI 重新啟動後客戶端不需修改設定即可重連。
- GUI 中的用戶端區域按用戶端名稱和版本歸併「最近活動」，不是即時 session 清單；工具
  醒目標示表示目前正在處理的呼叫。
- GUI 不可用時，本機 endpoint 也不可用，因為不存在繼續執行的獨立 MCP 行程。
- GUI 可用但 MCP 停用期間，新的 tool call 傳回結構化 `mcp_disabled` 錯誤；已啟動
  專案長任務仍可透過已驗證的 `tasks/get` 查詢或 `tasks/cancel` 取消。
- GUI 重新啟動後會再次綁定已設定的 loopback 連接埠，客戶端後續請求可以重連；工具是否
  傳回資料仍取決於 GUI 中的 MCP 啟用開關。
- 如果設定連接埠已被占用，MCP 頁面會顯示 server 未執行，技術日誌會記錄啟動錯誤。

## 錯誤與疑難排解

### `mcp_disabled`

MCP 頁面處於停用狀態。endpoint 仍可能顯示執行中，這是正常狀態；啟用 MCP 後重新呼叫
工具即可傳回資料。已經啟動的專案長任務是例外，客戶端仍可使用 `tasks/get`、
`tasks/cancel` 查詢結果或取消任務。

### `rate_limited`

內建 server 在短時間內收到過多 tool call，或已有 64 個 tool call 正在執行。每分鐘
最多啟動 600 次 tool call，達到限制後請稍後重試。

### The MCP endpoint is unavailable

常見原因：

- ALCOMD3 GUI 未執行。
- 客戶端 URL 與 GUI 目前顯示的 MCP Endpoint 不一致。
- 設定的本機連接埠已被占用。

處理方式：

1. 啟動 ALCOMD3。
2. 在 MCP 頁面確認 endpoint running。
3. 重新複製 MCP Endpoint 和授權權杖，更新 MCP 客戶端設定。
4. 重啟 MCP 客戶端。

Windows 上使用支援的用戶端時，可以再次點擊對應的快速設定按鈕，按提示確認取代，
然後完全結束並重新啟動該用戶端。

### HTTP `401 Unauthorized`

bearer token 缺失或與 ALCOMD3 顯示的 token 不一致。請更新用戶端的 `Authorization`
header。

### HTTP `403 Forbidden`

請求帶有不允許的瀏覽器 `Origin`。ALCOMD3 只接受原生 MCP 用戶端和相同 loopback origin，
以防止 DNS rebinding 和跨網站請求存取本機 server。

### Protocol negotiation errors

使用 MCP `2026-07-28` 時，每次請求都需攜帶標準 `MCP-Protocol-Version`、`Mcp-Method`
和 `_meta`；或者先初始化 `2025-11-25` legacy session，再執行一般工具呼叫。server 不
公布其他協定版本。

## 開發 smoke test

在倉庫根目錄建置包含內建 MCP 服務的 GUI：

```powershell
cargo build -p vrc-get-gui
```

執行 HTTP 生命週期和安全 smoke tests：

```powershell
cargo test -p vrc-get-gui mcp::
```

預期結果：

- `initialize` 成功。
- 攜帶標準 header 和請求 metadata 時，`2026-07-28` 的 `server/discover` 與一般
  無 session 請求成功。
- `2025-11-25` legacy session 可以初始化並執行一般工具呼叫。
- `tools/list` 傳回目前可用的 MCP 工具。
- `tools/call` 傳回 `ok: false` 的可讀錯誤，並在 MCP tool result 上標記
  `isError: true`。
- 缺少或錯誤 bearer token 會傳回 HTTP `401`。
- 不允許的 Origin 會傳回 HTTP `403`。

## 相關原始碼

- 內建 HTTP/RMCP 服務與工具：`vrc-get-gui/src/mcp/server.rs`
- MCP 生命週期、直接分派、操作與共享狀態：`vrc-get-gui/src/mcp/mod.rs`
- 內部 MCP 資料類型：`vrc-get-gui/src/mcp/types.rs`
- GUI 共享後端服務和 MCP capability 矩陣：`vrc-get-gui/src/backend/`
- GUI Tauri commands：`vrc-get-gui/src/commands/mcp.rs`
- GUI MCP 頁面：`vrc-get-gui/app/_main/mcp/index.tsx`
- 打包邏輯：`xtask/src/build_alcom.rs`、`xtask/src/bundle_alcom*`

## 參考

- RMCP 3.1.2: <https://github.com/modelcontextprotocol/rust-sdk/releases/tag/rmcp-v3.1.2>
- MCP Specification `2026-07-28`: <https://modelcontextprotocol.io/specification/2026-07-28>
- MCP Specification `2025-11-25`: <https://modelcontextprotocol.io/specification/2025-11-25>
- 實驗性 Tasks 擴充功能：<https://github.com/modelcontextprotocol/ext-tasks>
