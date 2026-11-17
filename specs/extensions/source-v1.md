# 单扩展源在线安装 v1

状态：M8 已批准合同，本地实现与定向验证中。与 VPM 相似的单扩展版本索引，不解析 VPM 清单，不提供升级或扩展市场。

同一匿名 HTTPS URL 返回 UTF-8 JSON，`format` 为 `alcomd-extension-source-v1`，`packages` 恰好一个合法 ExtensionId。扩展字段为 `displayName`（1–120 字符）、`publisherFingerprint`、可选 `description`（最多2048 UTF-8字节）和 `versions`；版本键为严格 SemVer，最多256项，每项为 `apiMajor`、绝对 HTTPS `url`、小写64位 `zipSHA256`。Schema 见 [source-v1.schema.json](source-v1.schema.json)。重复JSON键、未知字段、无效身份/版本/URL均拒绝，实际索引读取上限1MiB。不同 API 的版本可见，当前只能准备 API1。Core按SemVer降序返回，GUI默认最高受支持正式版，只有预发布时由用户选择。

```json
{"format":"alcomd-extension-source-v1","packages":{"dev.example.extension":{"displayName":"Example","publisherFingerprint":"ed25519-sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","versions":{"1.0.0":{"apiMajor":1,"url":"https://example.invalid/extension-1.0.0.alcomdextension","zipSHA256":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"}}}}}
```

| RPC / capability | 行为 |
| --- | --- |
| `extensions.inspectSource({url})` | 认证 local-owner＋extensions.manage；仅读取清单，返回URL、身份、名称/描述、版本/API及sourceFingerprint。无下载包、安装或授权。 |
| `extensions.planInstallFromSource({url,version,expectedSourceFingerprint})` | 同上；重新读取完整索引，先比较指纹，变化即既有revision_conflict，尚未下载。下载所选版本，复验ZIP摘要、现有签名/内容树/包/API及ID/版本/发布者一致，生成原不可变安装Plan。 |
| `extensions.remote-install.v1` | 两个新RPC的协商能力；不改现有WIT、权限名或扩展联网能力。 |

URL仅HTTPS，无userinfo/fragment；合法查询参数及顺序保持。最多10次HTTPS重定向，拒绝降级及带userinfo/fragment目标。请求匿名，不读取凭据存储，不发送认证头或Referer，沿用现有代理和全局下载并发/原子缓存。索引排队/请求各30秒；文件请求600秒、连接10秒，文件实际读取最多64MiB。失败和日志不带原始响应、认证信息或敏感URL查询。`extension_source_invalid` 表示格式/身份不匹配，`extension_source_unavailable` 表示连接/状态/TLS等失败，摘要及大小复用package_integrity_mismatch/package_download_too_large。签名等原稳定错误保持。

sourceFingerprint以域分隔SHA256绑定规范化请求URL及已校验、排序后的完整索引；JSON空白或对象顺序不改变指纹，地址查询顺序变化会改变。PlanFingerprint额外绑定包内容树和来源证据。State42新增Plan remote_source_json并扩展来源CHECK；保存清单URL、版本、索引指纹、下载URL和ZIP摘要，其余列/数据/外键和不可变触发条件保留。Apply前复核缓存ZIP与冻结摘要，继续执行原签名/身份/文件验证、Operation、幂等及恢复。安装详情只展示来源URL/版本，不暴露缓存路径。已安装冲突不替换现有包。

GUI工具栏“安装扩展”主按钮直接打开网址输入，右侧下箭头菜单提供“本地文件”；弹窗不再放来源分段。在线网址输入阶段底部仅有“取消／读取清单”，读取后选择版本，主操作变为“安装扩展”→原签名条款和必需权限两步确认（空步骤跳过）→原Apply/Operation。网址框Enter触发当前步骤，保留IME输入。拒绝不Apply，不授予权限；最终同意仅授予必需能力。准备阶段可关闭或Esc取消，含关闭动画期间阻断迟到结果；Core仍可能完成缓存或留下未应用Plan，取消不意味着回滚，也不会自动Apply。Apply接受后的任务沿用原规则，不因关闭连接取消；结果未知仍不盲目重试。

破坏性范围仅获批State42及新增来源/可选摘要；旧本地安装参数和行为保留，无更新/自动来源重规划旁路。现有原运行数据不升级。测试CA仅通过test-kill-gates私有构造器注入，正式入口始终使用正常受信任根，不写系统信任或关闭TLS验证。
