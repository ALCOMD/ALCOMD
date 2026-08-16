# ALCOMD3 3.4.0 更新 API 与 v4 bridge 基线

状态：端点与现有响应形状已核验；v4 bridge 尚未发布，bridge 版本推进契约未完成

最后联网核验：2026-08-16（Asia/Shanghai）

## 端点

```text
stable https://alcomd.cqmhv.com/api/v1/updates/stable.json
beta   https://alcomd.cqmhv.com/api/v1/updates/beta.json
```

ALCOMD3 3.4.0 的冻结配置将这两个端点作为后续更新源。更早版本先通过原更新链升级到
3.4.0；只有 3.4.0 可以作为 v4 的直接迁移来源。

v3 请求携带 `Accept: application/json`、`X-Alcom-Version`、`X-Alcom-OS`、
`X-Alcom-Arch`。HTTP 204 表示无更新；非 2xx、JSON/URL/semver 错误均为检查失败。

## 2026-08-16 在线快照

| 频道 | HTTP | 当前版本 | 发布日期 | UTF-8 响应 SHA-256 | 字节数 |
|---|---:|---|---|---|---:|
| stable | 200 | `3.3.0` | `2026-08-13T14:51:56Z` | `17b26180130b983941037f97481f1ca68ce4e1e127b027ed0c993cd5fff0554e` | 4603 |
| beta | 200 | `3.3.0-beta.2` | `2026-08-12T08:46:37Z` | `b99e5ab0b8c739ad887fb84f3e61d0247518610b2cf33cd8dffd543f530aa803` | 4386 |

当前响应尚未提供 v4 bridge。版本低于已安装的 3.4.0 时，3.4.0 必须保持运行且不得尝试
降级或进入迁移。这份在线摘要用于证明 M-1 核验时的端点状态；端点本身会随发行更新，
不能把当前响应哈希硬编码进生产客户端。

## 当前 JSON 形状

顶层字段：

```text
version
notes
notes_i18n
pub_date
platforms
```

平台键当前为：

```text
windows-x86_64
darwin-aarch64
linux-x86_64
```

每个平台对象至少包含：

```text
signature
url
args
```

签名是由冻结 updater 公钥验证的公开发行签名；`args` 是平台安装参数而不是任意 shell
命令。实现必须把平台键、URL scheme、版本、签名和参数类型作为不可信输入验证。

v3 runtime 的精确可观察规则：

- `version` 必填，也接受别名 `name`；必须是合法 semver 且严格大于当前版本。
- `platforms` 和当前平台 entry 必填，entry 的 `url`、`signature` 必填；`notes` 与
  `notes_i18n` 可选。生成器写入的 `pub_date`、`args` 及其他未知字段由 v3 runtime 忽略。
- stable staging 拒绝 prerelease；beta 接受 stable/prerelease；频道值仅 `stable|beta`。
- 资产由 v3 内嵌 Minisign 公钥验证。冻结指纹为
  `sha256:b0a37092fcf14677d503b2b9cea74c59d3c91de09eda2b5cb6db1d710af5b146`，key id
  为 `DDEF8C83EF8404B0`。authenticated trusted comment 必须包含精确
  `file:<expected basename>` 与 `purpose:release`。
- 文件名必须匹配 `ALCOMD3_{version}_windows_x86_64_setup.exe`、
  `ALCOMD3_{version}_macos_aarch64.app.tar.gz` 或
  `ALCOMD3_{version}_linux_x86_64.AppImage.tar.gz`。Windows 最大 256 MiB，macOS/Linux
  最大 512 MiB。
- Windows payload 必须接受 `/SP- /SILENT /NOICONS` 与原 scope 对应的
  `/CURRENTUSER|/ALLUSERS`。v3 只确认 ShellExecute 接受后即退出，并不等待 bridge 最终结果。
- macOS 在原位置替换 app bundle，Linux 在原位置替换 AppImage，v3 均不保留长期恢复副本。
  DEB 禁用 self-updater，必须有独立 package-manager 迁移入口。
- 下载后和安装前都重新验签。确定性安装失败写 `failed.json` 并丢包；瞬时 I/O 失败保留
  staging 以便重试。

manifest JSON 自身没有签名或 host pinning；真正的资产真实性依赖内嵌公钥。首个 bridge
资产必须继续由 v3 key 签名，不能先切换 v4 key。

## v4 完整产品 bridge installer 必须冻结的契约

v3.4.0 只发现、验签并启动 bridge，不执行完整替换迁移。bridge 是安装完整 v4 产品并包含或
调用 `alcomd-bootstrap` 的安装包。在发布 bridge 前必须以 JSON Schema、正负 Fixture 和
3.4.0 验证代码共同冻结：

- 哪个版本/显式字段表示普通 v3 更新与不可逆的 v4 bridge，不得仅凭 URL 或文件名猜测。
- stable/beta 频道如何保持映射，beta 是否允许回到 stable，以及 prerelease 比较规则。
- 三平台资产、签名、公钥轮换、摘要、重定向和允许的安装参数。
- 未知字段、缺字段、重复字段、无效时间、无效 URL、平台缺失和版本回退行为。
- 204、非 2xx、超时、无效 JSON、错误 content type、错误签名和部分下载时的恢复行为。
- 只有 bridge 验证完成后才能启动 `alcomd-bootstrap`；失败时不得修改 v3 数据、安装登记或
  创建可被误认为有效的 v4 状态。
- bridge 自己必须在替换/退出 v3 前建立可验证恢复源、持久 journal、幂等 retry、Health
  Check 和 Commit marker。Windows 启动 bridge 后以及 macOS/Linux 替换 executable 后的每个
  故障点，都必须证明 v3 数据未丢且 v3.4.0 可重新启动。
- 需要另行冻结 Windows mutex/提升取消与 scope 保持、临时 updater 精确清理责任、stable/beta
  rollout 与 kill switch、DEB 交接、迁移失败后的唯一状态持有者及新 v4 updater key 启用点。

## 阶段边界

M-1 只冻结端点、JSON、版本、签名和失败契约，不实现或发布 bridge。bridge、bootstrap 和
迁移程序属于 M11；正式签名材料与真实更新端点写入仍需发布阶段人工操作。
