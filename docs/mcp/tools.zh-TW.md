# ALCOMD3 MCP 工具參考

[English](tools.md) | [簡體中文](tools.zh-CN.md) | [日本語](tools.ja.md)

本檔案以目前公開的 33 個 MCP 工具實現為準，適合在編寫 Agent、排查呼叫或檢查相容性時查閱。
連接、驗證、生命週期和客戶端設定請參閱 [MCP 主指南](mcp.zh-TW.md)。

## 如何閱讀

- 輸入欄位使用 `snake_case`，輸出欄位通常使用 `camelCase`；請以每個工具的表格為準。
- “必填”為“是”的欄位必須傳入；“否”表示可以省略。省略後的預設行為寫在欄位說明中。
- 無輸入欄位的工具仍應傳入空物件 `{}`。
- `string \| null` 表示欄位始終存在，但目前值可能為空；“出現條件”則說明欄位是否只在特定情況下出現。
- 運行時的 `tools/list` 會提供每個工具的 `inputSchema`。目前
  `alcomd3_list_repositories` 還提供嚴格的 `outputSchema`。
- 工具傳回的是 MCP `structuredContent`。大多數成功結果包含 `ok: true`，但
  `alcomd3_get_activity_log_entry` 和 `alcomd3_get_technical_log_entry`
  直接傳回詳細資料物件，不包含 `ok`；下文逐項註明。

業務錯誤統一傳回：

```json
{
    "ok": false,
    "error": {
        "code": "error_code",
        "message": "Actionable message",
        "data": {}
    }
}
```

其中 `error.data` 僅在錯誤需要附帶結構化上下文時出現，MCP 外層結果同時帶有
`isError: true`。參數結構錯誤、業務錯誤和協議錯誤的區別請參閱
[MCP Tools 規範](https://modelcontextprotocol.io/specification/2025-11-25/server/tools#error-handling)。

## 快速索引

| 分類 | 工具 | 行為 | 用途 |
| --- | --- | --- | --- |
| 專案 | `alcomd3_list_projects` | 只讀 | 列出已登錄專案。 |
| 範本 | `alcomd3_list_templates` | 只讀 | 列出可用的環境層級範本。 |
| 範本 | `alcomd3_get_template` | 只讀 | 讀取一個環境層級範本。 |
| 範本 | `alcomd3_create_template` | 寫入 | 建立衍生範本。 |
| 範本 | `alcomd3_edit_template` | 破壞性寫入 | 透過整體替換定義來編輯衍生範本。 |
| 範本 | `alcomd3_set_template_package` | 冪等寫入 | 設定一個直接 VPM 相依。 |
| 範本 | `alcomd3_remove_template_package` | 破壞性寫入 | 移除一個直接 VPM 相依。 |
| 範本 | `alcomd3_set_template_unitypackage` | 冪等寫入 | 設定一個 UnityPackage 附件引用。 |
| 範本 | `alcomd3_remove_template_unitypackage` | 破壞性寫入 | 移除一個 UnityPackage 附件引用。 |
| 範本 | `alcomd3_remove_template` | 破壞性寫入 | 將可刪除範本移入資源回收筒。 |
| 專案 | `alcomd3_get_project_details` | 只讀 | 讀取已登錄專案詳細資料。 |
| 倉庫 | `alcomd3_list_repositories` | 只讀 | 列出遠端倉庫和軟體包顯示設定。 |
| 倉庫 | `alcomd3_add_repository` | 外部網路寫入 | 新增遠端 VPM 倉庫。 |
| 倉庫 | `alcomd3_remove_repository` | 破壞性寫入 | 按 URL 刪除使用者倉庫。 |
| 軟體包 | `alcomd3_get_package_details` | 只讀 | 讀取可見軟體包詳細元資料。 |
| 軟體包 | `alcomd3_list_packages` | 只讀 | 分頁列出所有 GUI 可見軟體軟體包摘要。 |
| 軟體包 | `alcomd3_list_repository_packages` | 只讀 | 分頁列出一個倉庫的軟體軟體包摘要。 |
| 環境 | `alcomd3_get_environment_settings` | 只讀 | 讀取 Unity 安裝和預設路徑設定。 |
| 活動記錄 | `alcomd3_search_activity_logs` | 只讀 | 篩選並分頁讀取活動摘要。 |
| 活動記錄 | `alcomd3_get_activity_log_entry` | 只讀 | 讀取一筆完整活動記錄。 |
| 活動記錄 | `alcomd3_summarize_activity_logs` | 只讀 | 聚合活動記錄。 |
| 活動記錄 | `alcomd3_get_activity_log_context` | 只讀 | 讀取一筆活動前後的上下文。 |
| 技術日誌 | `alcomd3_search_technical_logs` | 只讀 | 篩選並分頁讀取技術日誌預覽。 |
| 技術日誌 | `alcomd3_get_technical_log_entry` | 只讀 | 讀取一筆技術日誌詳細資料。 |
| 技術日誌 | `alcomd3_summarize_technical_logs` | 只讀 | 聚合技術日誌。 |
| 專案 | `alcomd3_create_project` | 長任務寫入 | 建立並登錄 Unity 專案。 |
| 專案 | `alcomd3_add_existing_project` | 寫入 | 登錄已有 Unity 專案。 |
| 專案 | `alcomd3_backup_project` | 長任務寫入 | 建立專案 zip 備份。 |
| 專案 | `alcomd3_copy_project` | 長任務寫入 | 複製並登錄專案。 |
| 專案 | `alcomd3_restore_project_from_backup` | 長任務寫入 | 從 zip 還原並登錄專案。 |
| 專案軟體包 | `alcomd3_install_project_package` | 長任務寫入 | 安裝一個 VPM 軟體包。 |
| 專案軟體包 | `alcomd3_uninstall_project_package` | 破壞性長任務 | 卸載一個已安裝包。 |
| 專案軟體包 | `alcomd3_reinstall_project_package` | 長任務寫入 | 重裝一個已安裝包。 |

“長任務”表示工具宣告 `execution.taskSupport: "optional"`：支援 MCP Tasks 的客戶端可以非同步輪詢，
不支援 Tasks 的客戶端仍可用普通同步 `tools/call`。完整行為見
[專案長任務](mcp.zh-TW.md#專案長任務)。

## 專案和範本

### `alcomd3_list_projects`

列出 ALCOMD3 資料庫中登錄的專案，不掃描未登錄目錄。

**輸入：** 無欄位，傳 `{}`。

**成功輸出：**

| 欄位 | 類型 | 出現條件 | 含義 |
| --- | --- | --- | --- |
| `ok` | `boolean` | 必有 | 成功時為 `true`。 |
| `projects` | [`ProjectSummary[]`](#projectsummary) | 必有 | 已登錄專案摘要；無法形成有效路徑摘要的記錄會被跳過。 |

### `alcomd3_list_templates`

列出目前可用於建立專案的環境層級範本。這些範本不是某個已登錄專案擁有的範本資料。範本來源檔案路徑不會傳回。

**輸入：** 無欄位，傳 `{}`。

**成功輸出：**

| 欄位 | 類型 | 出現條件 | 含義 |
| --- | --- | --- | --- |
| `ok` | `boolean` | 必有 | 成功時為 `true`。 |
| `templates` | [`TemplateSummary[]`](#templatesummary) | 必有 | 範本摘要和能力標記。 |

### `alcomd3_get_template`

按穩定範本 ID 讀取一個環境層級範本；此呼叫不會檢查已登錄專案。ID 僅用於選擇讀取物件，不會使只讀呼叫變成寫入。

**輸入：**

| 欄位 | 類型 | 必填 | 含義 |
| --- | --- | --- | --- |
| `template_id` | `string` | 是 | `alcomd3_list_templates` 傳回的範本 `id`；去除首尾空白後不能為空。 |

**成功輸出：**

| 欄位 | 類型 | 出現條件 | 含義 |
| --- | --- | --- | --- |
| `ok` | `boolean` | 必有 | 成功時為 `true`。 |
| `template` | [`TemplateDetails`](#templatedetails) | 必有 | 範本摘要和可讀取定義；不包含範本儲存路徑。 |

### `alcomd3_create_template`

建立一個衍生範本。後端產生並持久化範本 ID。

**輸入：**

| 欄位 | 類型 | 必填 | 含義 |
| --- | --- | --- | --- |
| `display_name` | `string` | 是 | 使用者可見範本名稱。 |
| `base_template_id` | `string` | 是 | 必須指向 `usableAsBase: true` 的現有範本。 |
| `unity_version_range` | `string` | 是 | 可解析的 Unity 版本範圍。 |
| `vpm_dependencies` | `object<string, string>` | 是 | VPM 軟體包名稱到版本範圍的完整對應。 |
| `unitypackage_paths` | `string[]` | 是 | 已存在的絕對 `.unitypackage` 一般檔案路徑。可傳空陣列。 |

**成功輸出：**

| 欄位 | 類型 | 出現條件 | 含義 |
| --- | --- | --- | --- |
| `ok` | `boolean` | 必有 | 成功時為 `true`。 |
| `template` | [`TemplateDetails`](#templatedetails) | 必有 | 新建範本的持久化定義和產生 ID。 |

附件只被引用，不會複製。自引用、基礎範本相依環、無效軟體包名稱、無效版本範圍和無效附件路徑會被拒絕。

### `alcomd3_edit_template`

整體替換一個衍生範本的可編輯定義；範本 ID 和儲存位置保持不變。

**輸入：**

| 欄位 | 類型 | 必填 | 含義 |
| --- | --- | --- | --- |
| `template_id` | `string` | 是 | 要編輯的衍生範本 ID。 |
| `display_name` | `string` | 是 | 替換後的顯示名稱。 |
| `base_template_id` | `string` | 是 | 替換後的基礎範本 ID。 |
| `unity_version_range` | `string` | 是 | 替換後的 Unity 版本範圍。 |
| `vpm_dependencies` | `object<string, string>` | 是 | 替換後的完整 VPM 相依對應。 |
| `unitypackage_paths` | `string[]` | 是 | 替換後的完整附件路徑列表。 |

**成功輸出：**

| 欄位 | 類型 | 出現條件 | 含義 |
| --- | --- | --- | --- |
| `ok` | `boolean` | 必有 | 成功時為 `true`。 |
| `template` | [`TemplateDetails`](#templatedetails) | 必有 | 編輯後的完整定義。 |

內建範本和專案歸檔範本不能欄位級編輯。本工具標記為 destructive，因為採用整體替換語義。

### `alcomd3_set_template_package`

為衍生範本設定一個直接 VPM 相依。它只儲存軟體包名稱和版本範圍宣告，不選擇倉庫、不解析相依，也不安裝檔案。

**輸入：**

| 欄位 | 類型 | 必填 | 含義 |
| --- | --- | --- | --- |
| `template_id` | `string` | 是 | 可編輯的衍生範本 ID。 |
| `package_name` | `string` | 是 | 合法的完整 VPM 軟體包名稱。 |
| `version_range` | `string` | 是 | 要新增或替換的可解析 VPM 版本範圍。 |

**成功輸出：** `ok: true`，並在 `template` 中傳回完整的最新 [`TemplateDetails`](#templatedetails)。重複設定相同軟體包名稱和範圍不會寫入。

### `alcomd3_remove_template_package`

從衍生範本移除一個直接 VPM 相依宣告，不會修改任何既有專案。

**輸入：**

| 欄位 | 類型 | 必填 | 含義 |
| --- | --- | --- | --- |
| `template_id` | `string` | 是 | 可編輯的衍生範本 ID。 |
| `package_name` | `string` | 是 | 要移除的現有直接相依。 |

**成功輸出：** `ok: true`，並在 `template` 中傳回完整的最新 [`TemplateDetails`](#templatedetails)。相依不存在時傳回 `template_package_not_found`。

### `alcomd3_set_template_unitypackage`

為衍生範本設定一個 UnityPackage 附件引用。

**輸入：**

| 欄位 | 類型 | 必填 | 含義 |
| --- | --- | --- | --- |
| `template_id` | `string` | 是 | 可編輯的衍生範本 ID。 |
| `unitypackage_path` | `string` | 是 | 已存在的絕對 `.unitypackage` 一般檔案路徑。 |

路徑會被規範化，檔案只會被引用而不會複製。重複設定同一規範路徑不會寫入。

**成功輸出：** `ok: true`，並在 `template` 中傳回完整的最新 [`TemplateDetails`](#templatedetails)。

### `alcomd3_remove_template_unitypackage`

從衍生範本移除一個 UnityPackage 附件引用。路徑應複製自 `alcomd3_get_template`；引用的檔案不會被刪除，也不要求仍然存在。

**輸入：**

| 欄位 | 類型 | 必填 | 含義 |
| --- | --- | --- | --- |
| `template_id` | `string` | 是 | 可編輯的衍生範本 ID。 |
| `unitypackage_path` | `string` | 是 | 範本定義中已有的附件路徑。 |

**成功輸出：** `ok: true`，並在 `template` 中傳回完整的最新 [`TemplateDetails`](#templatedetails)。引用不存在時傳回 `template_unitypackage_not_found`。

### `alcomd3_remove_template`

把一個可刪除範本移入系統資源回收筒。內建範本不可刪除，附件檔案不會被刪除。

**輸入：**

| 欄位 | 類型 | 必填 | 含義 |
| --- | --- | --- | --- |
| `template_id` | `string` | 是 | 要刪除的範本 ID。 |

**成功輸出：**

| 欄位 | 類型 | 出現條件 | 含義 |
| --- | --- | --- | --- |
| `ok` | `boolean` | 必有 | 成功時為 `true`。 |
| `template` | [`RemovedTemplate`](#removedtemplate) | 必有 | 被移除範本的識別碼、名稱和類型。 |

### `alcomd3_get_project_details`

讀取一個已登錄專案的 Unity 資訊和已安裝包。不能借此讀取任意未登錄目錄。

**輸入：**

| 欄位 | 類型 | 必填 | 含義 |
| --- | --- | --- | --- |
| `project_path` | `string` | 是 | 必須精確匹配 ALCOMD3 資料庫中的已登錄專案路徑。 |

**成功輸出：**

| 欄位 | 類型 | 出現條件 | 含義 |
| --- | --- | --- | --- |
| `ok` | `boolean` | 必有 | 成功時為 `true`。 |
| `project` | [`ProjectDetails`](#projectdetails) | 必有 | Unity 版本、解析狀態和已安裝包。 |

### `alcomd3_create_project`

建立 Unity 專案、解析專案軟體包並登錄到 ALCOMD3。支援可選 MCP Task。

**輸入：**

| 欄位 | 類型 | 必填 | 含義 |
| --- | --- | --- | --- |
| `project_name` | `string` | 是 | 單個合法目錄名，不能是路徑、根目錄或 `..`。 |
| `base_path` | `string` | 否 | 絕對父目錄；省略時使用 GUI 預設專案目錄。 |
| `template_id` | `string` | 否 | 範本 ID；省略時遵循 GUI 目前範本選擇規則。 |
| `unity_version` | `string` | 否 | Unity 版本；省略時遵循 GUI 目前範本選擇規則。 |

**成功輸出：**

| 欄位 | 類型 | 出現條件 | 含義 |
| --- | --- | --- | --- |
| `ok` | `boolean` | 必有 | 成功時為 `true`。 |
| `projectPath` | `string` | 必有 | 新專案的絕對路徑。 |
| `templateId` | `string` | 必有 | 實際使用的範本 ID。 |
| `unityVersion` | `string` | 必有 | 實際選擇的 Unity 版本。 |

### `alcomd3_add_existing_project`

把已有 Unity 專案登錄到 ALCOMD3，不複製專案內容。

**輸入：**

| 欄位 | 類型 | 必填 | 含義 |
| --- | --- | --- | --- |
| `project_path` | `string` | 是 | 指向有效 Unity 專案目錄的絕對路徑。 |

**成功輸出：**

| 欄位 | 類型 | 出現條件 | 含義 |
| --- | --- | --- | --- |
| `ok` | `boolean` | 必有 | 成功時為 `true`。 |
| `projectPath` | `string` | 必有 | 實際登錄的專案路徑。 |

### `alcomd3_backup_project`

為已登錄專案建立 zip 備份。支援可選 MCP Task。

**輸入：**

| 欄位 | 類型 | 必填 | 含義 |
| --- | --- | --- | --- |
| `project_path` | `string` | 是 | 已登錄專案路徑。 |
| `backup_name` | `string` | 否 | 不含 `.zip` 的單一合法檔案名；省略時自動產生。不能傳路徑。 |
| `exclude_vpm_packages` | `boolean` | 否 | 為 `true` 時排除已安裝 VPM 軟體包內容；預設 `false`。 |

**成功輸出：**

| 欄位 | 類型 | 出現條件 | 含義 |
| --- | --- | --- | --- |
| `ok` | `boolean` | 必有 | 成功時為 `true`。 |
| `backupPath` | `string` | 必有 | 建立出的 zip 絕對路徑。 |

備份總是寫入 GUI 設定的備份目錄，不覆蓋已有檔案。

### `alcomd3_copy_project`

複製一個已登錄專案，並登錄複製後的專案。支援可選 MCP Task。

**輸入：**

| 欄位 | 類型 | 必填 | 含義 |
| --- | --- | --- | --- |
| `source_project_path` | `string` | 是 | 已登錄源專案路徑。 |
| `new_project_path` | `string` | 是 | 尚不存在的絕對目標目錄，且不能位于源專案內。 |

**成功輸出：**

| 欄位 | 類型 | 出現條件 | 含義 |
| --- | --- | --- | --- |
| `ok` | `boolean` | 必有 | 成功時為 `true`。 |
| `projectPath` | `string` | 必有 | 複製並登錄後的專案路徑。 |

### `alcomd3_restore_project_from_backup`

從 zip 備份還原專案到 GUI 預設專案目錄並登錄。支援可選 MCP Task。

**輸入：**

| 欄位 | 類型 | 必填 | 含義 |
| --- | --- | --- | --- |
| `backup_path` | `string` | 是 | ALCOMD3 zip 備份的絕對檔案路徑。 |
| `project_name` | `string` | 否 | 還原後的單一合法目錄名；省略時使用備份檔案名。 |

**成功輸出：**

| 欄位 | 類型 | 出現條件 | 含義 |
| --- | --- | --- | --- |
| `ok` | `boolean` | 必有 | 成功時為 `true`。 |
| `projectPath` | `string` | 必有 | 還原並登錄後的專案路徑。 |

## 倉庫、軟體包和環境

### `alcomd3_list_repositories`

列出所有受支援的遠端倉庫以及影響軟體包可見性的全域設定。本機倉庫不受支援。

**輸入：** 無欄位，傳 `{}`。

**成功輸出：**

| 欄位 | 類型 | 出現條件 | 含義 |
| --- | --- | --- | --- |
| `ok` | `boolean` | 必有 | 成功時為 `true`。 |
| `repositories` | [`RepositorySummary[]`](#repositorysummary) | 必有 | 官方、Curated 和使用者遠端倉庫的唯一規範陣列。 |
| `packageVisibility` | `object` | 必有 | 全域軟體軟體包顯示設定。 |
| `packageVisibility.hideLocalUserPackages` | `boolean` | 必有 | 是否隱藏本機使用者軟體包。 |
| `packageVisibility.showPrereleasePackages` | `boolean` | 必有 | 是否顯示預發布軟體包。 |

包讀取工具使用傳回的 `id`；刪除使用者倉庫使用傳回的 `url`。

### `alcomd3_add_repository`

下載、驗證並新增一個遠端 VPM 倉庫，然後清理軟體包快取。此工具會存取倉庫 URL。

**輸入：**

| 欄位 | 類型 | 必填 | 含義 |
| --- | --- | --- | --- |
| `repository_url` | `string` | 是 | 有效的遠端 VPM 倉庫 URL，也是後續刪除使用的身分。 |
| `headers` | `object<string, string>` | 否 | 下載倉庫時附帶的 HTTP header 對應；預設空物件。 |

**成功輸出：**

| 欄位 | 類型 | 出現條件 | 含義 |
| --- | --- | --- | --- |
| `ok` | `boolean` | 必有 | 成功時為 `true`。 |
| `repository` | [`RepositoryMutationSummary`](#repositorymutationsummary) | 必有 | 實際加入的使用者倉庫摘要。 |

已存 URL 或發布者宣告 ID 重復都會被拒絕。活動記錄只保存脫敏 URL 和 header 數量，不保存 header 值。

### `alcomd3_remove_repository`

按已存 URL 精確刪除一個使用者倉庫並清理軟體包快取。預設倉庫不能刪除。

**輸入：**

| 欄位 | 類型 | 必填 | 含義 |
| --- | --- | --- | --- |
| `repository_url` | `string` | 是 | `alcomd3_list_repositories` 傳回的使用者倉庫 `url`。只接受 URL，不接受 ID。 |

**成功輸出：**

| 欄位 | 類型 | 出現條件 | 含義 |
| --- | --- | --- | --- |
| `ok` | `boolean` | 必有 | 成功時為 `true`。 |
| `repository` | [`RepositoryMutationSummary`](#repositorymutationsummary) | 必有 | 被刪除倉庫的摘要。 |

### `alcomd3_list_packages`

分頁列出與 GUI 包列表相同的可見軟體軟體包摘要。工具不提供服務端文本搜尋。

**輸入：**

| 欄位 | 類型 | 必填 | 含義 |
| --- | --- | --- | --- |
| `offset` | `integer >= 0` | 否 | 起始條目偏移；預設 `0`。 |
| `limit` | `integer >= 0` | 否 | 要求頁大小；預設 `200`，實際限制在 `1..=1000`。 |

**成功輸出：**

| 欄位 | 類型 | 出現條件 | 含義 |
| --- | --- | --- | --- |
| `ok` | `boolean` | 必有 | 成功時為 `true`。 |
| `totalCount` | `integer` | 必有 | 篩選並按來源聚合後的摘要總數。 |
| `offset` | `integer` | 必有 | 本次使用的偏移。 |
| `limit` | `integer` | 必有 | 本次實際頁大小。 |
| `returnedCount` | `integer` | 必有 | 本頁傳回數量。 |
| `hasMore` | `boolean` | 必有 | 是否還有下一頁。 |
| `nextOffset` | `integer \| null` | 必有 | 下一頁偏移；無下一頁時為 `null`。 |
| `packages` | [`PackageSummary[]`](#packagesummary) | 必有 | 目前頁的軟體軟體包摘要。 |

### `alcomd3_list_repository_packages`

分頁列出一個遠端倉庫中的 GUI 可見軟體軟體包摘要。

**輸入：**

| 欄位 | 類型 | 必填 | 含義 |
| --- | --- | --- | --- |
| `repository_id` | `string` | 是 | `alcomd3_list_repositories` 傳回的倉庫 `id`。只接受 ID，不接受 URL。 |
| `offset` | `integer >= 0` | 否 | 起始偏移；預設 `0`。 |
| `limit` | `integer >= 0` | 否 | 頁大小；預設 `200`，實際限制在 `1..=1000`。 |

**成功輸出：** 與 `alcomd3_list_packages` 的分頁欄位相同，並額外包含：

| 欄位 | 類型 | 出現條件 | 含義 |
| --- | --- | --- | --- |
| `repository` | [`PackageRepositorySummary`](#packagerepositorysummary) | 必有 | 被讀取倉庫的摘要。 |
| `packages` | [`PackageSummary[]`](#packagesummary) | 必有 | 僅來自指定倉庫的目前頁摘要。 |

### `alcomd3_get_package_details`

讀取一個 GUI 可見軟體包的詳細元資料。省略篩選欄位時可能傳回多個來源或版本。

**輸入：**

| 欄位 | 類型 | 必填 | 含義 |
| --- | --- | --- | --- |
| `package_name` | `string` | 是 | 完整 VPM 軟體軟體包識別碼；去除首尾空白後不能為空。 |
| `version` | `string` | 否 | 精確版本字串。 |
| `repository_id` | `string` | 否 | 將結果限制到指定遠端倉庫；使用倉庫列表傳回的 ID。只接受 ID。 |

**成功輸出：**

| 欄位 | 類型 | 出現條件 | 含義 |
| --- | --- | --- | --- |
| `ok` | `boolean` | 必有 | 成功時為 `true`。 |
| `packages` | [`PackageDetails[]`](#packagedetails) | 必有 | 所有匹配且 GUI 可見的軟體包詳細資料，至少一項。 |

### `alcomd3_get_environment_settings`

讀取 ALCOMD3 目前保存的 Unity 安裝、啟動參數和預設路徑；不啟動 Unity，也不掃描額外磁碟。

**輸入：** 無欄位，傳 `{}`。

**成功輸出：**

| 欄位 | 類型 | 出現條件 | 含義 |
| --- | --- | --- | --- |
| `ok` | `boolean` | 必有 | 成功時為 `true`。 |
| `unityInstallations` | [`UnityInstallation[]`](#unityinstallation) | 必有 | 已登錄 Unity 安裝。 |
| `unityLaunchArguments` | `object` | 必有 | Unity 啟動參數來源和有效值。 |
| `unityLaunchArguments.configured` | `string[] \| null` | 必有 | 使用者設定；未設定時為 `null`。 |
| `unityLaunchArguments.builtinDefault` | `string[]` | 必有 | ALCOMD3 內建預設參數。 |
| `unityLaunchArguments.effective` | `string[]` | 必有 | 目前實際生效的參數。 |
| `unityLaunchArguments.usesBuiltinDefault` | `boolean` | 必有 | 是否正在使用內建預設值。 |
| `paths` | `object` | 必有 | 預設目錄。 |
| `paths.defaultProjectPath` | `string` | 必有 | 預設專案目錄。 |
| `paths.projectBackupPath` | `string` | 必有 | 專案備份目錄。 |

## 活動記錄

### 活動篩選公共輸入

`alcomd3_search_activity_logs` 和 `alcomd3_summarize_activity_logs` 共用以下篩選欄位：

| 欄位 | 類型 | 必填 | 含義 |
| --- | --- | --- | --- |
| `search` | `string` | 否 | 在 operation、summary、target、tool name 和 client name 中做不區分大小寫的包含匹配。 |
| `sources` | `("gui" \| "mcp" \| "deep_link" \| "system")[]` | 否 | 限制活動來源。 |
| `kinds` | `("read" \| "write" \| "passive" \| "open" \| "maintenance")[]` | 否 | 限制活動類型。 |
| `statuses` | `("started" \| "succeeded" \| "failed" \| "cancelled" \| "info")[]` | 否 | 限制活動狀態。 |
| `visibility` | `"important" \| "primary" \| "secondary" \| "technical" \| "all"` | 否 | 可見性層級；預設 `important`。 |
| `operations` | `string[]` | 否 | 限制內部 operation 識別碼。 |
| `tool_names` | `string[]` | 否 | 限制 MCP 工具名。 |
| `request_id` | `string` | 否 | 限制 MCP 請求 ID。 |
| `target` | `string` | 否 | 限制操作物件。 |
| `since` | `RFC3339 string` | 否 | 包含的最早時間。 |
| `until` | `RFC3339 string` | 否 | 包含的最晚時間；不得早于 `since`。 |
| `offset` | `integer >= 0` | 否 | 分頁偏移；預設 `0`。 |
| `limit` | `integer >= 0` | 否 | 頁大小；預設 `50`，實際限制在 `1..=200`。 |
| `order` | `"newest" \| "oldest"` | 否 | 時間順序；預設 `newest`。 |

### `alcomd3_search_activity_logs`

按公共篩選條件分頁讀取使用者可讀活動摘要。

**輸入：** [活動篩選公共輸入](#活動篩選公共輸入)中的任意欄位；全部可選，可傳 `{}`。

**成功輸出：**

| 欄位 | 類型 | 出現條件 | 含義 |
| --- | --- | --- | --- |
| `ok` | `boolean` | 必有 | 成功時為 `true`。 |
| `totalCount` | `integer` | 必有 | 篩選後的活動總數。 |
| `offset`、`limit`、`returnedCount`、`hasMore`、`nextOffset` | 分頁欄位 | 必有 | 語義與軟體包分頁相同。 |
| `entries` | [`ActivityEntrySummary[]`](#activityentrysummary) | 必有 | 目前頁活動摘要。 |

### `alcomd3_get_activity_log_entry`

按搜尋或彙總結果中的 ID 讀取一筆完整活動記錄。

**輸入：**

| 欄位 | 類型 | 必填 | 含義 |
| --- | --- | --- | --- |
| `id` | `string` | 是 | 活動記錄 ID。 |
| `include_details` | `boolean` | 否 | 是否傳回 `details`；預設 `true`。為 `false` 時傳回空陣列。 |

**成功輸出：** 直接傳回 [`ActivityEntry`](#activityentry)，不包含 `ok` 包裝欄位。

### `alcomd3_summarize_activity_logs`

按欄位聚合篩選後的活動，用於先定位問題範圍。

**輸入：** 公共篩選欄位，另加：

| 欄位 | 類型 | 必填 | 含義 |
| --- | --- | --- | --- |
| `group_by` | `"source" \| "kind" \| "status" \| "operation" \| "tool_name" \| "client_name" \| "day" \| "hour"` | 否 | 聚合維度；預設 `source`。 |

**成功輸出：**

| 欄位 | 類型 | 出現條件 | 含義 |
| --- | --- | --- | --- |
| `ok` | `boolean` | 必有 | 成功時為 `true`。 |
| `groupBy` | `string` | 必有 | 實際聚合維度。 |
| `totalCount` | `integer` | 必有 | 篩選後的活動總數。 |
| `totalGroupCount` | `integer` | 必有 | 分頁前的分組總數。 |
| `offset`、`limit`、`returnedCount`、`hasMore`、`nextOffset` | 分頁欄位 | 必有 | 對分組列表分頁。 |
| `groups` | [`ActivitySummaryGroup[]`](#activitysummarygroup) | 必有 | 目前頁聚合結果。 |

### `alcomd3_get_activity_log_context`

讀取指定活動及其相鄰記錄，不需要拉取全部日誌。

**輸入：**

| 欄位 | 類型 | 必填 | 含義 |
| --- | --- | --- | --- |
| `id` | `string` | 是 | 中心活動記錄 ID。 |
| `before` | `integer >= 0` | 否 | 前置記錄數量；預設 `5`，最大 `50`。 |
| `after` | `integer >= 0` | 否 | 後置記錄數量；預設 `5`，最大 `50`。 |
| `include_details` | `boolean` | 否 | 是否在三組記錄中包含詳細資料；預設 `false`。 |

**成功輸出：**

| 欄位 | 類型 | 出現條件 | 含義 |
| --- | --- | --- | --- |
| `ok` | `boolean` | 必有 | 成功時為 `true`。 |
| `entry` | [`ActivityEntry`](#activityentry) | 必有 | 中心記錄。 |
| `before` | [`ActivityEntry[]`](#activityentry) | 必有 | 中心記錄之前的活動。 |
| `after` | [`ActivityEntry[]`](#activityentry) | 必有 | 中心記錄之後的活動。 |

## 技術日誌

### 技術日誌篩選公共輸入

`alcomd3_search_technical_logs` 和 `alcomd3_summarize_technical_logs` 共用：

| 欄位 | 類型 | 必填 | 含義 |
| --- | --- | --- | --- |
| `search` | `string` | 否 | 在 target 和 message 中做不區分大小寫的包含匹配。 |
| `levels` | `("error" \| "warn" \| "info" \| "debug" \| "trace")[]` | 否 | 日誌級別；預設 `error` 和 `warn`。 |
| `targets` | `string[]` | 否 | target 的不區分大小寫包含匹配。 |
| `scope` | `"memory" \| "recent_files"` | 否 | 目前進程記憶體或近期日誌檔案；預設 `memory`。 |
| `since` | `RFC3339 string` | 否 | 包含的最早時間。 |
| `until` | `RFC3339 string` | 否 | 包含的最晚時間；不得早于 `since`。 |
| `offset` | `integer >= 0` | 否 | 分頁偏移；預設 `0`。 |
| `limit` | `integer >= 0` | 否 | 頁大小；預設 `50`，實際限制在 `1..=100`。 |
| `max_message_chars` | `integer >= 0` | 否 | 搜尋預覽最多字元數；預設且最大為 `300`。彙總結果不含訊息文本。 |

### `alcomd3_search_technical_logs`

分頁讀取已脫敏、有限長度的技術日誌預覽。

**輸入：** [技術日誌篩選公共輸入](#技術日誌篩選公共輸入)中的任意欄位；全部可選。

**成功輸出：**

| 欄位 | 類型 | 出現條件 | 含義 |
| --- | --- | --- | --- |
| `ok` | `boolean` | 必有 | 成功時為 `true`。 |
| `totalCount` | `integer` | 必有 | 篩選後的日誌總數。 |
| `offset`、`limit`、`returnedCount`、`hasMore`、`nextOffset` | 分頁欄位 | 必有 | 目前頁狀態。 |
| `entries` | [`TechnicalLogEntrySummary[]`](#technicallogentrysummary) | 必有 | 目前頁技術日誌預覽。 |

### `alcomd3_get_technical_log_entry`

讀取搜尋結果中的一筆日誌，訊息會先脫敏再截斷。

**輸入：**

| 欄位 | 類型 | 必填 | 含義 |
| --- | --- | --- | --- |
| `id` | `string` | 是 | 搜尋結果傳回的技術日誌 ID。 |
| `max_message_chars` | `integer >= 0` | 否 | 訊息最多字元數；預設且最大為 `4000`。 |

**成功輸出：** 直接傳回 [`TechnicalLogEntryDetails`](#technicallogentrydetails)，不包含 `ok` 包裝欄位。

### `alcomd3_summarize_technical_logs`

聚合篩選後的技術日誌。

**輸入：** 技術日誌公共篩選欄位，另加：

| 欄位 | 類型 | 必填 | 含義 |
| --- | --- | --- | --- |
| `group_by` | `"level" \| "target" \| "file" \| "hour"` | 否 | 聚合維度；預設 `level`。 |

**成功輸出：**

| 欄位 | 類型 | 出現條件 | 含義 |
| --- | --- | --- | --- |
| `ok` | `boolean` | 必有 | 成功時為 `true`。 |
| `groupBy` | `string` | 必有 | 實際聚合維度。 |
| `totalCount` | `integer` | 必有 | 篩選後的日誌總數。 |
| `totalGroupCount` | `integer` | 必有 | 分頁前的分組總數。 |
| `offset`、`limit`、`returnedCount`、`hasMore`、`nextOffset` | 分頁欄位 | 必有 | 對分組分頁。 |
| `groups` | [`TechnicalLogSummaryGroup[]`](#technicallogsummarygroup) | 必有 | 目前頁聚合結果。 |

## 專案軟體包寫入

### `alcomd3_install_project_package`

從 GUI 可見且與專案 Unity 版本相容的候選中安裝一個包。支援可選 MCP Task。

**輸入：**

| 欄位 | 類型 | 必填 | 含義 |
| --- | --- | --- | --- |
| `project_path` | `string` | 是 | 已登錄專案路徑。 |
| `package_name` | `string` | 是 | 合法的完整 VPM 軟體軟體包識別碼。 |
| `version_selector` | `object` | 是 | `{"type":"latest_gui_visible"}`，或 `{"type":"exact","version":"x.y.z"}`。精確版本仍須 GUI 可見且相容。 |
| `source` | `object` | 否 | 可選來源選擇器；空物件等同省略。 |
| `source.repository_id` | `string` | 否 | 遠端倉庫 ID。 |
| `source.repository_url` | `string` | 否 | 遠端倉庫 URL。若 ID 和 URL 同時給出，必須匹配同一倉庫。 |
| `allow_conflicts` | `boolean` | 否 | 是否允許相依衝突或 legacy 檔案/目錄刪除；預設 `false`。 |

**成功輸出：** [`ProjectPackageChangeResult`](#projectpackagechangeresult)。

### `alcomd3_uninstall_project_package`

卸載一個已安裝包。支援可選 MCP Task，並標記為 destructive。

**輸入：**

| 欄位 | 類型 | 必填 | 含義 |
| --- | --- | --- | --- |
| `project_path` | `string` | 是 | 已登錄專案路徑。 |
| `package_name` | `string` | 是 | 目前專案中已安裝的合法 VPM 軟體包識別碼。 |
| `allow_conflicts` | `boolean` | 否 | 是否允許衝突或 legacy 刪除；預設 `false`。 |

**成功輸出：** [`ProjectPackageChangeResult`](#projectpackagechangeresult)。

### `alcomd3_reinstall_project_package`

重新安裝一個已安裝包。支援可選 MCP Task。

**輸入：** 與 `alcomd3_uninstall_project_package` 相同。

**成功輸出：** [`ProjectPackageChangeResult`](#projectpackagechangeresult)。

三個工具都會先產生 pending changes。若需要明確授權但 `allow_conflicts` 為 `false`，傳回
`project_package_conflicts`，並在 `error.data.changes` 中提供同一 [`PendingChanges`](#pendingchanges)
結構；此時不會修改專案。

## 共享輸出類型

### `ProjectSummary`

| 欄位 | 類型 | 含義 |
| --- | --- | --- |
| `name` | `string \| null` | 專案顯示名稱。 |
| `path` | `string` | 登錄路徑。 |
| `projectType` | `string` | 後端識別的專案類型。 |
| `unity` | `string \| null` | Unity 版本。 |
| `unityRevision` | `string \| null` | Unity revision。 |
| `lastModified` | `integer \| null` | 最後修改 Unix 毫秒時間。 |
| `createdAt` | `integer \| null` | 建立 Unix 毫秒時間。 |
| `favorite` | `boolean` | 是否收藏。 |
| `exists` | `boolean` | 登錄目錄目前是否存在。 |

### `TemplateSummary`

| 欄位 | 類型 | 含義 |
| --- | --- | --- |
| `displayName` | `string` | 範本顯示名稱。 |
| `id` | `string` | 穩定管理 ID。 |
| `unityVersions` | `string[]` | 可供專案建立選擇的 Unity 版本。 |
| `updateDate` | `string \| null` | 範本更新時間。 |
| `hasUnityPackages` | `boolean` | 是否引用 Unity package。 |
| `hasProjectArchive` | `boolean` | 是否包含專案歸檔。 |
| `available` | `boolean` | 目前範本是否可用。 |
| `kind` | `"builtIn" \| "derived" \| "projectArchive"` | 範本類型。 |
| `editable` | `boolean` | 是否可欄位級編輯。 |
| `removable` | `boolean` | 是否可刪除。 |
| `usableAsBase` | `boolean` | 是否可作為衍生範本的基礎範本。 |

### `TemplateDetails`

包含全部 [`TemplateSummary`](#templatesummary) 欄位，另有：

| 欄位 | 類型 | 含義 |
| --- | --- | --- |
| `baseTemplateId` | `string \| null` | 衍生範本的基礎範本 ID。 |
| `unityVersionRange` | `string \| null` | 衍生範本的 Unity 版本範圍。 |
| `vpmDependencies` | `object<string, string>` | VPM 軟體包名稱到版本範圍。 |
| `unityPackagePaths` | `string[]` | 被引用的絕對附件路徑。 |

### `RemovedTemplate`

| 欄位 | 類型 | 含義 |
| --- | --- | --- |
| `id` | `string` | 被刪除範本 ID。 |
| `displayName` | `string` | 被刪除範本名稱。 |
| `kind` | `"builtIn" \| "derived" \| "projectArchive"` | 被刪除範本類型。 |

### `ProjectDetails`

| 欄位 | 類型 | 含義 |
| --- | --- | --- |
| `path` | `string` | 專案路徑。 |
| `unity.major` | `integer` | Unity 主版本。 |
| `unity.minor` | `integer` | Unity 次版本。 |
| `unity.version` | `string` | 完整 Unity 版本。 |
| `unity.revision` | `string \| null` | Unity revision。 |
| `shouldResolve` | `boolean` | 專案是否需要重新解析包。 |
| `installedPackages` | `object[]` | 已安裝包列表。 |
| `installedPackages[].id` | `string` | 專案相依項中的包 ID。 |
| `installedPackages[].package` | [`PackageDetails`](#packagedetails) | 已安裝 manifest 摘要，不含 `source`。 |

### `RepositorySummary`

| 欄位 | 類型 | 含義 |
| --- | --- | --- |
| `id` | `string` | 包讀取工具使用的倉庫身分；無宣告 ID 時回退到 URL。 |
| `url` | `string` | 遠端倉庫 URL；使用者倉庫刪除時使用此值。 |
| `displayName` | `string` | 顯示名稱。 |
| `kind` | `"officialDefault" \| "curatedDefault" \| "user"` | 唯一倉庫分類欄位。 |
| `hidden` | `boolean` | 目前是否被 GUI 軟體包可見性設定隱藏。 |

### `RepositoryMutationSummary`

| 欄位 | 類型 | 含義 |
| --- | --- | --- |
| `id` | `string \| null` | 倉庫宣告 ID；缺失時可能為 `null`。 |
| `url` | `string` | 已新增或刪除的 URL。 |
| `displayName` | `string \| null` | 倉庫顯示名稱。 |
| `kind` | `"user"` | 固定為使用者倉庫。 |

### `PackageRepositorySummary`

| 欄位 | 類型 | 含義 |
| --- | --- | --- |
| `id` | `string \| null` | 快取倉庫 ID。 |
| `url` | `string \| null` | 快取倉庫 URL。 |
| `displayName` | `string \| null` | 顯示名稱。 |
| `kind` | `"officialDefault" \| "curatedDefault" \| "user"` | 倉庫分類。 |

### `PackageSummary`

| 欄位 | 類型 | 含義 |
| --- | --- | --- |
| `name` | `string` | 完整 VPM 軟體軟體包識別碼。 |
| `displayName` | `string \| null` | 顯示名稱。 |
| `version` | `string` | 版本。 |
| `source` | [`PackageSource`](#packagesource) | 軟體包來源。 |

### `PackageSource`

遠端來源：

| 欄位 | 類型 | 含義 |
| --- | --- | --- |
| `type` | `"remote"` | 表示包來自遠端倉庫。 |
| `kind` | `"officialDefault" \| "curatedDefault" \| "userRepository"` | 遠端倉庫分類。 |
| `id` | `string \| null` | 倉庫宣告 ID。 |
| `displayName` | `string \| null` | 倉庫顯示名稱。 |
| `url` | `string \| null` | 倉庫 URL。 |

本機使用者軟體軟體包來源：

| 欄位 | 類型 | 含義 |
| --- | --- | --- |
| `type` | `"localUser"` | 表示包來自已登錄的本機使用者軟體包目錄。 |
| `kind` | `"localUser"` | 固定的本機使用者軟體包分類。 |
| `isLocalUserPackage` | `true` | 明確標記這是本機使用者軟體包。 |

### `PackageDetails`

| 欄位 | 類型 | 含義 |
| --- | --- | --- |
| `name` | `string` | 完整軟體包識別碼。 |
| `displayName` | `string \| null` | 顯示名稱。 |
| `description` | `string \| null` | 包描述。 |
| `version` | `string` | 版本。 |
| `unity` | `object \| null` | Unity 要求，非空時含 `major`、`minor`。 |
| `keywords` | `string[]` | 關鍵詞。 |
| `aliases` | `string[]` | 軟體包別名。 |
| `vpmDependencies` | `string[]` | 相依軟體包識別碼列表，不包含版本範圍。 |
| `legacyPackages` | `string[]` | 被取代的 legacy 包。 |
| `changelogUrl` | `string \| null` | 變更日誌 URL。 |
| `documentationUrl` | `string \| null` | 文檔 URL。 |
| `isYanked` | `boolean` | 目前版本是否撤回。 |
| `source` | [`PackageSource`](#packagesource) | 軟體包詳細資料工具傳回時必有；專案已安裝軟體包摘要中不出現。 |

### `ActivityEntrySummary`

| 欄位 | 類型 | 含義 |
| --- | --- | --- |
| `id` | `string` | 活動記錄 ID，可用於詳細資料和上下文工具。 |
| `startedAt` | `RFC3339 string` | 活動開始時間。 |
| `finishedAt` | `RFC3339 string \| null` | 活動完成時間；尚未完成時為 `null`。 |
| `source` | `string` | 活動來源，例如 `Gui`、`Mcp`、`DeepLink` 或 `System`。 |
| `kind` | `string` | 行為類型，例如 `Read`、`Write` 或 `Maintenance`。 |
| `status` | `string` | 目前或最終狀態，例如 `Started`、`Succeeded`、`Failed`。 |
| `importance` | `string` | 可見性級別：`Primary`、`Secondary` 或 `Technical`。 |
| `operation` | `string` | 穩定的內部操作識別碼。 |
| `summary` | `string` | 面向使用者的簡短活動說明。 |
| `target` | `string \| null` | 被操作的資源或路徑。 |
| `durationMs` | `integer \| null` | 已完成活動的耗時毫秒數。 |
| `requestId` | `string \| null` | 關聯的 MCP 請求 ID。 |
| `toolName` | `string \| null` | 關聯的 MCP 工具名。 |
| `clientName` | `string \| null` | 發起呼叫的 MCP 客戶端名稱。 |
| `detailCount` | `integer` | 完整記錄中鍵值詳細資料的數量。 |
| `hasError` | `boolean` | 完整記錄是否包含錯誤文本。 |
| `errorSummary` | `string \| null` | 已脫敏、截斷的錯誤摘要。 |

### `ActivityEntry`

| 欄位 | 類型 | 含義 |
| --- | --- | --- |
| `id` | `string` | 活動記錄 ID。 |
| `source` | `string` | 活動來源；目前輸出使用 `Gui`、`Mcp`、`DeepLink`、`System`。 |
| `kind` | `string` | 行為類型；目前輸出使用 `Read`、`Write`、`Passive`、`Open`、`Maintenance`。 |
| `status` | `string` | 活動狀態；目前輸出使用 `Started`、`Succeeded`、`Failed`、`Cancelled`、`Info`。 |
| `importance` | `string` | 可見性級別：`Primary`、`Secondary` 或 `Technical`。 |
| `operation` | `string` | 穩定的內部操作識別碼。 |
| `summary` | `string` | 面向使用者的活動說明。 |
| `target` | `string \| null` | 操作物件。 |
| `details` | [`ActivityDetail[]`](#activitydetail) | 已脫敏的結構化詳細資料。傳 `include_details: false` 時為空陣列。 |
| `requestId` | `string \| null` | 關聯的 MCP 請求 ID。 |
| `toolName` | `string \| null` | 關聯的 MCP 工具名。 |
| `clientName` | `string \| null` | MCP 客戶端名稱。 |
| `startedAt` | `RFC3339 string` | 活動開始時間。 |
| `finishedAt` | `RFC3339 string \| null` | 活動完成時間。 |
| `durationMs` | `integer \| null` | 活動耗時毫秒數。 |
| `error` | `string \| null` | 已脫敏的完整錯誤文本。 |

輸入篩選列舉使用小寫 `snake_case`；上表列出的輸出列舉使用目前實現值。

### `ActivityDetail`

| 欄位 | 類型 | 含義 |
| --- | --- | --- |
| `key` | `string` | 詳細資料鍵。 |
| `value` | `string` | 已脫敏的詳細資料值。 |

### `ActivitySummaryGroup`

| 欄位 | 類型 | 含義 |
| --- | --- | --- |
| `key` | `string` | 分組鍵。 |
| `count` | `integer` | 記錄數。 |
| `failedCount` | `integer` | 失敗數。 |
| `cancelledCount` | `integer` | 取消數。 |
| `latestEntryId` | `string \| null` | 組內最新記錄 ID。 |
| `latestStartedAt` | `string \| null` | 組內最新開始時間。 |

### `TechnicalLogEntrySummary`

| 欄位 | 類型 | 含義 |
| --- | --- | --- |
| `id` | `string` | 技術日誌 ID，可用於詳細資料工具。 |
| `time` | `RFC3339 string` | 日誌時間。 |
| `level` | `string` | 目前輸出使用 `Error`、`Warn`、`Info`、`Debug` 或 `Trace`。 |
| `target` | `string` | 產生日誌的 Rust target。 |
| `messagePreview` | `string` | 已脫敏並按搜尋上限截斷的訊息預覽。 |
| `truncated` | `boolean` | 訊息是否被截斷。 |
| `source` | `"memory" \| "file"` | 日誌來自目前進程記憶體還是近期日誌檔案。 |
| `fileName` | `string \| null` | 來源日誌檔案名；記憶體日誌為 `null`。 |
| `lineNumber` | `integer \| null` | 來源日誌檔案行號；記憶體日誌為 `null`。 |

### `TechnicalLogEntryDetails`

| 欄位 | 類型 | 含義 |
| --- | --- | --- |
| `id` | `string` | 技術日誌 ID。 |
| `time` | `RFC3339 string` | 日誌時間。 |
| `level` | `string` | `Error`、`Warn`、`Info`、`Debug` 或 `Trace`。 |
| `target` | `string` | 產生日誌的 Rust target。 |
| `message` | `string` | 已脫敏並按詳細資料要求上限截斷的訊息。 |
| `truncated` | `boolean` | 訊息是否被截斷。 |
| `source` | `"memory" \| "file"` | 日誌來源。 |
| `fileName` | `string \| null` | 來源檔案名；記憶體日誌為 `null`。 |
| `lineNumber` | `integer \| null` | 來源檔案行號；記憶體日誌為 `null`。 |

### `TechnicalLogSummaryGroup`

| 欄位 | 類型 | 含義 |
| --- | --- | --- |
| `key` | `string` | 分組鍵。 |
| `count` | `integer` | 日誌數。 |
| `errorCount` | `integer` | Error 數量。 |
| `warnCount` | `integer` | Warn 數量。 |
| `latestEntryId` | `string \| null` | 組內最新日誌 ID。 |
| `latestTime` | `string \| null` | 組內最新時間。 |

### `ProjectPackageChangeResult`

| 欄位 | 類型 | 含義 |
| --- | --- | --- |
| `ok` | `boolean` | 成功時為 `true`。 |
| `operation` | `"install" \| "uninstall" \| "reinstall"` | 實際操作。 |
| `projectPath` | `string` | 被修改專案路徑。 |
| `packageName` | `string` | 目標軟體包識別碼。 |
| `changes` | [`PendingChanges`](#pendingchanges) | 已應用變更摘要。 |

### `PendingChanges`

| 欄位 | 類型 | 含義 |
| --- | --- | --- |
| `changes_version` | `integer` | 後端變更快照版本。 |
| `package_changes` | `[string, PackageChange][]` | 軟體包名稱與安裝/移除變更的元組列表。 |
| `remove_legacy_files` | `string[]` | 將刪除的 legacy 檔案。 |
| `remove_legacy_folders` | `string[]` | 將刪除的 legacy 目錄。 |
| `conflicts` | `[string, ConflictInfo][]` | 軟體包名稱與衝突詳細資料的元組列表。 |

`PackageChange` 是 `{ "InstallNew": PackageInfo }` 或
`{ "Remove": "Requested" \| "Legacy" \| "Unused" }`。`Remove` 值分別表示使用者要求刪除、
被其他軟體包取代或已不再被相依。

### `PackageInfo`

| 欄位 | 類型 | 含義 |
| --- | --- | --- |
| `name` | `string` | 要安裝的完整軟體包識別碼。 |
| `display_name` | `string \| null` | 軟體包顯示名稱。 |
| `description` | `string \| null` | 軟體包描述。 |
| `keywords` | `string[]` | 合併後的軟體包別名和關鍵詞。 |
| `version` | `object` | 結構化 SemVer，包含 `major`、`minor`、`patch`、`pre`、`build`。 |
| `unity` | `[integer, integer] \| null` | 所需 Unity 主版本和次版本。 |
| `changelog_url` | `string \| null` | 變更記錄 URL。 |
| `documentation_url` | `string \| null` | 文件 URL。 |
| `vpm_dependencies` | `string[]` | 相依軟體包識別碼。 |
| `legacy_packages` | `string[]` | 被取代的 legacy 軟體包。 |
| `is_yanked` | `boolean` | 該版本是否已撤回。 |

### `ConflictInfo`

| 欄位 | 類型 | 含義 |
| --- | --- | --- |
| `packages` | `string[]` | 與目標變更衝突的軟體包識別碼。 |
| `unity_conflict` | `boolean` | 是否存在 Unity 版本衝突。 |
| `unlocked_names` | `string[]` | 套用變更時需要解鎖的軟體包識別碼。 |

### `UnityInstallation`

| 欄位 | 類型 | 含義 |
| --- | --- | --- |
| `path` | `string` | Unity 可執行檔案路徑。 |
| `version` | `string` | 已登錄的完整 Unity 版本。 |
| `loadedFromHub` | `boolean` | 此安裝是否由 Unity Hub 記錄載入。 |
