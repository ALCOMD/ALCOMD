# v3 迁移工具

`alcomd-migrate-v3` 作为临时迁移工具独立构建。正常运行和 v4 安装产物排除旧解析器。

v3 派生模板只转换 Unity `2022.3.22f1` 的工程内容。模板和基底链都允许该版本时，导入生成一份独立模板。只允许其他版本的派生项保留在源目录，`inventory` 和 `export` 通过标准错误报告跳过数量。已经归档的独立工程保留其实际 Unity 版本。

先退出 v3.4.0，再向工具提供已确认属于该安装的绝对数据目录。源目录和项目保持原位。`inventory` 只输出记录数量。`export` 将摘要清单、原生状态和转换后的模板归档写入当前用户私有暂存父目录下一个此前不存在的目录。Bootstrap 核验桥接载荷和平台健康报告的签名，并协调数据暂存、Core 重开对账、提交和回滚。Windows 冻结更新器已验证原 v3 密钥签名交接。Windows 两种安装范围、Linux AppImage 和 DEB 已验证隔离原生数据提交。[M11 验收记录](../../docs/exec-plans/M11-v3-migration-and-zip.md#2026-11-24-windows-stable-双范围签名交接)

```text
alcomd-migrate-v3 inventory --source-root <绝对的已确认源目录>
alcomd-migrate-v3 export --source-root <绝对的已确认源目录> --output <绝对的新暂存目录>
```

使用 `--transfer-credentials` 可明确选择把支持的仓库认证请求头写入当前用户凭据库。导出文件只记录仓库是否配置过认证，以及已转移凭据的引用。省略该选项时，导出仍可完成。Core 导入含认证仓库的包时要求有效凭据引用，缺少引用会停止迁移。旧 MCP 令牌、授权、活动和技术日志均排除在数据包之外。实际清理须在 Bootstrap 提交点之后，按获批的具体实例清单执行。

`manifest.json` 记录 `state.json` 和每个模板归档的大小及 SHA-256。模板归档在迁移时转换成现行 `.alcomdtemplate` ZIP。目录记录见 `artifacts.toml`，原生包定义见 `schemas/`。

## Linux 产品桥接

下载并核对完整 v4 程序包和与它配对的签名迁移载荷。退出正在使用的 v3 和 v4 进程。用需要迁移资料的普通账号运行下列步骤。`DATA_PARENT` 是当前账号的绝对数据父目录，默认是 `${XDG_DATA_HOME:-$HOME/.local/share}`。`SOURCE` 是已经核实的 v3.4.0 资料目录。它和 `PAYLOAD` 都使用绝对路径。

AppImage 使用不同的文件路径保留旧版和新版。旧版可由冻结更新器的归档解出，也可保留原下载。两者字节相同。运行新版时显式进入桥接入口。

执行新版前，可用已独立验签的 `alcomd-updater` 核对外层完整 AppImage 的签名声明、字节数与 SHA-256。`OFFER` 为对应频道的 v4 签名更新声明，`RELEASE_HOST` 为获批的下载域名。桥接运行时还会核对内层产品载荷和迁移载荷的配对。

```sh
alcomd-updater verify-product-update --envelope "$OFFER" --asset "$NEW_APPIMAGE" \
    --channel stable --target x86_64-unknown-linux-gnu --format appimage \
    --installed-version 3.4.0 --allow-host "$RELEASE_HOST"
```

```sh
"$NEW_APPIMAGE" --bridge-v3 \
    --source-root "$SOURCE" --recovery-executable "$OLD_APPIMAGE" \
    --destination "$DATA_PARENT/alcomd" --work-directory "$DATA_PARENT/.alcomd-v3-migration" \
    --staged-payload "$PAYLOAD" --channel stable
```

DEB 先用 APT 安装 v4，保留已经配置好的 v3.4.0 和原始 `.deb`。系统包安装和账号迁移分别执行。`V4_VERSION` 使用所安装产品的实际版本。

```sh
"/usr/lib/alcomd/runtime/$V4_VERSION/alcomd-bootstrap" bridge-linux-v3 --install-scope deb \
    --source-root "$SOURCE" --recovery-executable /usr/bin/alcomd3 --recovery-package "$OLD_DEB" \
    --destination "$DATA_PARENT/alcomd" --work-directory "$DATA_PARENT/.alcomd-v3-migration" \
    --staged-payload "$PAYLOAD" --channel stable
```

首次调用输出待核对报告并保留暂存。核对通过后，用同一命令追加 `--health-report` 和 `--health-signature` 对应的签名证明。两次均使用原发行频道。现有迁移日志阻止普通 AppImage 和 DEB 首启在确认前创建空数据目录。提交后会打开正常 v4 桌面。提交点之前，可用配套 Bootstrap 的 `rollback-v3 --work-directory "$DATA_PARENT/.alcomd-v3-migration"` 撤回。需要重做时先将已完成回滚的日志归档到同一账号的私有目录。旧 AppImage 和旧 DEB 都保持原位。撤回已部署的 DEB 或批准移除一个已核实的 v3 包时，由 APT 或 dpkg 处理。

迁移器的自动化测试用合成 v3 配置、LiteDB 页和模板验证解析与原子导出。真实脱敏样本按 `fixtures/README.md` 采集和登记。LiteDB 页格式参考 [LiteDB v5 HeaderPage](https://github.com/litedb-org/LiteDB/blob/v5.0.21/LiteDB/Engine/Pages/HeaderPage.cs)、[BasePage](https://github.com/litedb-org/LiteDB/blob/v5.0.21/LiteDB/Engine/Pages/BasePage.cs) 与 [DataBlock](https://github.com/litedb-org/LiteDB/blob/v5.0.21/LiteDB/Engine/Structures/DataBlock.cs)。

## 外部 MCP 客户端配置预览

将需要核对的 v3 `gui-config.json` 和一个外部客户端配置复制到同一个隔离目录。命令使用明确传入的两个相对路径，并核对目录与文件身份。

```text
alcomd-migrate-v3 preview-mcp-client --fixture-root <绝对隔离目录> --v3-gui-config <相对 gui-config.json> --client <codex|claude-code|cursor> --client-config <相对客户端文件>
```

冻结的 3.4.0 Windows 快速设置使用已保存的 `mcpHttpPort`、`127.0.0.1` 上的 HTTP MCP 和固定环境变量引用。预览只核对已知 `alcomd3` 项的精确字段，分别报告 `exactV3Transport`、`absent` 或 `unverified`。所有结果的 `ownershipVerified` 均为 `false`，因为手工配置也能复现相同字段。命令只读取隔离夹具并输出客户端和分类。修改外部配置、撤销仍由 v3 管理的令牌或清理旧实例须另外确认归属并获授权。[冻结的快速设置实现](https://github.com/ALCOMD/ALCOMD3/blob/4aa98ae4f18d42c10137278997180dbede991e88/vrc-get-gui/src/mcp_client_config.rs)

保留了同一份文件在冻结版快速设置前后的副本时，可在隔离目录准备精确删除旧条目的补丁。前置快照必须来自已核实的快速设置操作。工具检查前置快照中旧条目尚未出现、当前条目精确匹配冻结版，并核对余下字段与快照一致。预览输出 `planDigest`。用相同输入与 `--apply-plan <planDigest>` 应用。日志目录必须是夹具根的单层目录名。

```text
alcomd-migrate-v3 patch-mcp-client --fixture-root <绝对隔离目录> --v3-gui-config <相对 gui-config.json> --client <codex|claude-code|cursor> --client-config <相对当前文件> --before-quick-setup <相对快速设置前快照> --journal <目录名>
alcomd-migrate-v3 rollback-mcp-client --fixture-root <绝对隔离目录> --journal <目录名> --plan-digest <planDigest>
```

补丁保留其他条目与原有格式。事务目录保留回滚所需的原始字节。发生中断时使用同一摘要恢复，检测到外部更改时保留冲突文件及事务记录。实际用户配置需要独立实例证据和授权，新版 MCP 客户端使用 v4 的登记和凭据签发流程。

## 其他已发布来源的选择性导入

`select` 独立于 v3.4.0 直接升级。以下是各来源目前接受的类别。

| 来源 | `--source-root` | 可选类别 |
| --- | --- | --- |
| VCC | 用户数据根下的 `VRChatCreatorCompanion` | `projects`、`resources`、`settings`、`theme`、`templates` |
| 旧 ALCOM | 用户数据根下的 `ALCOM` | `projects`、`resources`、`settings`、`theme`、`templates` |
| ALCOMD3 2.1.0 Beta | 包含 `vrc-get/` 的 ALCOMD3 数据根 | `settings`、`theme`、`templates` |

先预览并审查所选类别的摘要，再传入该次输出的 `planDigest` 确认执行。预览只读取所选来源及类别。应用前按同一来源和类别重算摘要，匹配后连接当前系统用户已经运行的 Core。

```text
alcomd-migrate-v3 select --source vcc --source-root <绝对源目录> --category projects
alcomd-migrate-v3 select --source vcc --source-root <绝对源目录> --category projects --apply-plan <planDigest>
alcomd-migrate-v3 select --source alcomd3-beta --source-root <绝对源目录> --category theme
```

- `projects` 从 `settings.json` 和存在时的 `vcc.liteDb` 读取项目及 Unity 登记。已有项目登记保持原样。对本次新登记的项目保留受支持的收藏及显式 Unity 启动参数。登记时间与安装信息由 Core 根据验收时间和实际文件记录。
- `resources` 登记 `settings.json` 中的无认证仓库和本地包。源仓库带认证时，此类别拒绝执行。
- `settings` 按当前修订更新在源中识别到的语言、全屏、VCC 链接、动效、备份压缩和默认 Unity 参数。VCC 与旧 ALCOM 的 `settings.json` 还可提供预发布包显示、默认项目目录和备份目录。
- `theme` 按当前修订更新外观模式。`settings` 和 `theme` 均保留导入字段之外的设置。
- `templates` 从 VCC 的 `Templates/` 工程目录或旧 ALCOM、Beta 的 `vrc-get/templates/` 导入。只登记当前 Core 支持的模板，已有不同内容的同标识模板保留原样。存档模板 Apply 需要提供源目录之外的 `--staging-directory <绝对目录>`。重试复用该目录和原摘要，等任务结束后可清理转换副本。

登记使用独立业务事务和固定的幂等键。应用中断时已接受的新增会保留，可以用仍匹配的摘要继续。官方 VCC 和使用 VCC 共用目录的旧 ALCOM 已通过默认资料复验。专用 `ALCOM` 目录和 ALCOMD3 2.1.0 Beta 已通过按冻结源码独立构造的非空样本补验。直接升级的原生包继续限定 `sourceVersion` 为 `3.4.0`。[选择性导入记录](../../docs/exec-plans/M11-v3-migration-and-zip.md#2026-11-24-专用-alcom-与-alcomd3-210-beta-自制样本)

[冻结的选择性导入定义](https://github.com/ALCOMD/ALCOMD3/blob/4aa98ae4f18d42c10137278997180dbede991e88/vrc-get-gui/src/commands/environment/legacy_import.rs) · [冻结的旧数据根定义](https://github.com/ALCOMD/ALCOMD3/blob/4aa98ae4f18d42c10137278997180dbede991e88/alcomd3-app-paths/src/lib.rs)
