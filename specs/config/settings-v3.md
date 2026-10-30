# settings.toml Config Schema 3

状态：M7 active production contract，已获项目所有者批准。

Config Schema 3 在既有 daemon 唯一 writer、strict UTF-8、16 KiB 上限、checked revision CAS、
same-directory `.new`/`.bak` 原子替换与恢复纪律之上，增加 `appearance.scheme`：

```toml
schema = 3
revision = 1
locale = "system"

[appearance]
mode = "system"
scheme = "vibrant"
density = "default"
motion = "system"

[packages]
show_prerelease = false
hidden_repository_ids = []
hide_local_user_packages = false
```

`scheme` 仅接受 `tonal-spot`、`fidelity`、`monochrome`、`neutral`、`vibrant`、`expressive`、
`content`、`rainbow`、`fruit-salad`。默认 `vibrant`；未知枚举和未知字段拒绝。
`source_color` 仍仅在非空时写出，并使用 canonical uppercase `#RRGGBB`。
scheme 是主题生成方式，不替代 source color、mode、density、motion、locale 或 package 设置。

`motion` 按所有者要求提供三态：`full`（启用动画，不受系统减少动态效果偏好影响）、
`reduced`（关闭动画）、`system`（跟随系统，默认值）。旧 `system`／`reduced` 值保持原意；
历史 Schema 1／2 不接受 `full`。此枚举增量归入尚未发布的 Config3，不增加 Config4、
RPC 方法、capability 或权限。严格旧客户端可能拒绝 `full`，属于本次用户要求的发布前
枚举合同增量，不提供旧客户端别名或双模型。partial update 省略表示不修改，未知值或
`null` 拒绝。动画选择与其他普通设置一样在修改时立即经既有 CAS RPC 保存，成功后更新运行时；
失败保留未确认意图、显示错误并提供重试，重试先重读，不能把未知提交结果当成未提交。
普通设置不再有全局 Save／Discard 栏；主题等有独立 Save／Cancel 的编辑器保留显式确认。
系统偏好变化只影响 `system` 的有效行为，不写入配置。
共享 UI 处理页面 CSS、Material Shadow DOM／菜单／弹窗和滚动条，不恢复已撤掉的切页动画。

启动迁移在 daemon ready 前执行：v2→v3 只补默认 scheme 并更新 schema；v1 先按已批准的
v1→v2 规则补 package 默认值，再进入 v3。保留原 revision 及其他所有有效用户设置；没有可靠的
scheme 历史值时使用 `vibrant`。迁移失败恢复有效输入 authority 并 fail closed；迁移不作为用户
修改推进 revision。只有成功的用户 `settings.update` 令 revision 加一。

`settings.get` 和成功的 `settings.update` 返回 `configSchema: 3`，完整归一化
`settings.appearance.scheme` 为必需字段。现有 partial `update.appearance.scheme` 可省略；省略保持
原值，提供时必须是九值之一，`null` 不是清除 scheme 的操作。三项主题重置通过同一次既有更新
将 `sourceColor: null`、`mode: "system"`、`scheme: "vibrant"` 写入，不引入新的重置方法。

`hidden_repository_ids` 继续最多 256 个 canonical lowercase UUID，unique、lexical byte order；
重复输入仍是 `invalid_request`。package presentation 不改变 resolver、Plan/Apply、来源或 cache。
没有新的 RPC method、capability、permission、State Schema 或生产依赖。

当前设置文件由 daemon 构建的 `alcomd-store::FileConfigStore` 适配器管理；application 的
`ConfigStore` port 只接受归一化快照，不接触文件路径、TOML 或替换工件。权限、窄更新校验、
revision CAS 和候选内存快照仍由同一个 application 用例持有；调用取消或磁盘发布结果不明
不得保留旧的已确认候选快照。该分层不改变上述存储格式、迁移顺序或持久化语义。

RPC partial update 的所有非 nullable 字段，包括 appearance／packages／unity 容器、locale 以及
各自子字段，提供显式 null 时整次拒绝，不部分应用同行的合法值。省略仍表示保持原值；
`sourceColor: null` 明确表示清除颜色，`unity.hubPath: null` 清除显式 Hub 路径并恢复默认发现。

## Unity Hub 位置

项目所有者已批准 [精确合同](unity-hub-location.proposal.contract.json)。当前未发布 Config3
增加可选 `[unity]` 的 `hub_path`，保存规范化后的 Hub 可执行文件路径；缺省及清除时省略
该字段和空 section，既有无 section 的 Config3 读取为 null。Schema 1／2 不接受该 section，
经既有迁移进入 Config3 后保持 null、原 revision 与其余偏好；不引入 Config4 或 State migration。
路径以 TOML basic string 保存，反斜杠及双引号可逆转义，Unicode 保留。路径最多 4096 UTF-8
字节，拒绝空串和控制字符；配置文件继续最多 16 KiB。由 application/platform 校验显式
更新的绝对路径与实际可执行文件，再经原有 ConfigStore 发布；无关设置更新不重新校验旧路径。

公开完整 `settings.unity.hubPath` 为 `string | null`。partial update 省略 unity/hubPath 均
保持原值；`hubPath:null` 清除，`unity:null`、空容器、未知字段拒绝。成功写入仍只推进一次
revision；无效路径与同行合法字段整次拒绝，不部分保存。首次发现、读取和路径状态变化不写
配置、不推进 revision。旧严格客户端须同步新字段，这是已批准的发布前响应扩展。

`settings.get/update` 在完整设置之外返回派生 `unityHub`：status 为
`available | not_found | unavailable`，source 为 `configured | discovered | none`，path 是可用
可执行文件路径或 null，configuredPathStatus 为 `unset | available | invalid | unavailable`。
可用的显式路径优先，否则按平台固定顺序最多检查八个默认候选；不递归扫描、不执行 Hub，
不进行注册表或 PATH 全盘探测。默认发现不替换已保存偏好。文件不可用与 I/O 不可判断分开，
派生证据不进入配置或候选内存快照；提交后证据读取失败不能把已保存设置报告成提交失败。
可用仅表示观察时文件可执行，不证明厂商身份或未来启动成功。

Windows 普通盘符/UNC 路径在确认仍解析到同一个 canonical 文件后返回普通路径格式，不将内部
verbatim 前缀直接用于常规路径展示；需要该语义的特殊设备/卷路径保持。GUI 不自行裁剪路径。
“自动查找”显式使用既有 `hubPath:null` 切回默认发现；已经为 null 时只重新读取 settings.get，
刷新同 revision 的派生证据，不推进 revision 或刷新 Editor registry。存在手动偏好时通过原串行
CAS 清除，确认成功后返回检测结果；未知提交先核对，不覆盖尚未展示的并发手动设置。

路径框允许直接编辑，草稿只属于当前 GUI；输入中不写权威设置，Enter 或焦点离开整个设置操作组时使用同一个串行
CAS/已有 Core 路径校验提交。Escape 恢复最新确认路径；无改动的自动发现值不会被保存成手动
覆盖。点击同组选择/查找动作不先提交未完成文本，校验失败保留输入及明确错误，成功才采用
Core 规范化结果。未找到/不可用文案仅为 placeholder，不作为路径值提交；IME 组合中的 Enter
不触发保存。无 RPC/Config 字段或权限增量。

显式清空已编辑文本仅显示本地必填提示，不发送空串 RPC；未配置/未检测到路径的空值仍是合法
自动模式，不标为全局必填。需要清除覆盖时使用既有“自动查找”。

GUI 选择后直接使用原串行 CAS 保存，取消不保存、不刷新 Editor；确认保存后仅当拥有
`unity.manage` 时调用已有 Editor refresh。Hub 配置／CLI 的 Editor 发现、Windows 注册表
探测及缺失 Editor 的安装链接仍是独立未闭合项，不能以该选择器完成冒充整个 Hub 功能完成。

已获批准的 `unity.hub.open` 是独立 application 命令，要求 `settings.read` 和
`unity.hub.open`，初期仅本地所有者可调用。它以已接受的 revision/effective path 核对当前
权威再启动，不保存配置或更改派生读取规则；无显式偏好且确实未找到 Hub 时返回
`download_required`，GUI 仅打开固定官方页面。该增量不增加 Config/State 字段、版本或迁移，
详细请求、错误及来源边界由 RPC 合同定义。

公开 DTO 与边界由 [机器合同](settings-v3.schema.json) 和
[RPC 合同](../rpc/m7-official-gui.schema.json) 冻结。Schema 1／2 合同保留为历史迁移输入，
不作为当前写入格式或公开结果。
