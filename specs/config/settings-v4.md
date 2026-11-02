# settings.toml Config Schema 4

状态：M7 当前唯一设置合同，已获项目所有者批准。Config1–3 仅保留为启动迁移输入。

Config4 删除紧凑模式及 `appearance.density`；当前文件、公开完整设置和 partial update 均不含该字段。
显式提供 `density`，包括旧 `default` / `compact` 值或 null，均作为未知字段整次拒绝；不提供别名、
隐藏开关、旧字段回退或第二套当前模型。其余设置语义、权限及持久化纪律保持。

```toml
schema = 4
revision = 1
locale = "system"

[appearance]
mode = "system"
scheme = "vibrant"
motion = "system"

[packages]
show_prerelease = false
hidden_repository_ids = []
hide_local_user_packages = false
```

`source_color` 非空时写出 canonical uppercase `#RRGGBB`；清除时省略。`scheme` 保留九值：
`tonal-spot`、`fidelity`、`monochrome`、`neutral`、`vibrant`、`expressive`、`content`、`rainbow`、
`fruit-salad`，默认 `vibrant`。mode 保留 `system | light | dark`；locale 保留
`system | en-US | zh-CN | ja-JP`。未知字段、未知枚举和非法值拒绝。

motion 保留 `full | reduced | system`，默认 `system`。`full` 不受系统减少动态效果偏好影响，
`reduced` 关闭动画，`system` 跟随系统；系统变化不写配置。Config1/2 的历史输入仍只接受原有
`system | reduced`，Config3 已有的 `full` 迁移后保留。共享 UI 继续处理 CSS、Material 弹窗/菜单和
滚动条，不恢复已移除的切页动画。

## 迁移、存储与公开 RPC

daemon ready 前按各自历史合同读取有效 Config1、2、3：丢弃旧 `appearance.density`，保留原 revision
及其余有效设置。Config1 仍补既有 packages 默认值及默认 scheme；Config2 仍只补默认 scheme；
Config3 已有 scheme、motion、package 偏好和显式 Hub 路径均保留。没有旧 Hub section 时维持 null。
迁移不算用户修改、不推进 revision；Config4 不再读取 density。失败恢复有效输入 authority 并 fail closed。

文件继续为 strict UTF-8、最多 16 KiB；`alcomd` 是唯一 writer，使用同目录同步 `.new` / `.bak`
可恢复原子替换。`alcomd-store::FileConfigStore` 管理文件；application 的 ConfigStore port 只接收
归一化快照，不操作 TOML 或路径。权限、checked revision CAS、字段校验和候选内存快照仍属既有 application。
取消或发布结果未知不得保留旧的已确认候选快照。

`system.hello` 在设置初始化成功后报告 `configSchema: 4`；`settings.get` 和成功的 `settings.update`
返回 Config4、完整归一化设置与派生 `unityHub`。删除 density 是已批准的发布前破坏性调整，严格客户端
须同步；RPC major、method、capability、permission、State Schema 与生产依赖不变。

完整 `appearance` 必须含 mode、scheme、sourceColor、motion；partial update 省略表示保持原值。
所有非 nullable 字段和 appearance/packages/unity 容器显式 null 或空容器均整次拒绝，不部分应用同行合法字段。
仅 `sourceColor: null` 清除颜色、`unity.hubPath: null` 清除显式 Hub 路径。主题重置仍通过一次既有更新
提交 `sourceColor: null`、`mode: system`、`scheme: vibrant`，不新增重置 RPC。

`settings.read/manage` 保持原权限；只有成功用户更新令 revision 加一。普通设置继续即时串行 CAS，
失败保留意图和明确错误，重试先重读核对未知提交；主题编辑器继续显式 Save/Cancel。
`hiddenRepositoryIds` 最多 256 个 canonical lowercase UUID，唯一并按 lexical byte order 排序；重复输入拒绝，
presentation 不改变 resolver、Plan/Apply、来源或 cache。

## Unity Hub 位置

既有 [Hub 合同](unity-hub-location.proposal.contract.json) 的能力继续有效。可选 `[unity]` 的 `hub_path`
保存显式可执行文件路径；缺省/清除时省略字段与空 section，公开 `settings.unity.hubPath` 为 `string | null`。
路径最多 4096 UTF-8 字节，拒绝空串/控制字符，以 TOML basic string 可逆转义反斜杠、引号并保留 Unicode。
显式更新仍由 Core 校验绝对路径、实际文件及可执行性；无关设置更新不重新校验旧路径。

派生 `unityHub` 保留 status、source、path、configuredPathStatus。可用显式路径优先，否则最多检查八个固定
默认候选；不递归扫描、不执行 Hub、不改变已保存偏好。读取及发现不写配置或推进 revision；成功提交后的
证据检测失败返回 unavailable，不把已提交设置改报失败。可用证据不证明厂商身份或未来启动成功。

GUI 的只读标签及完整路径 tooltip 复用 `displayPath`；输入、Core 路径、身份和 RPC/文件参数保持原值。
编辑草稿只在本地，Enter/离开整个操作组时用原串行 CAS 保存；Escape 恢复已确认路径，组合输入不误提交。
显式清空草稿显示本地必填提示而不发送空串；“自动查找”用既有 null 清除手动偏好，结果未知先核对。
取消选择不保存；已确认的路径选择只在具备 `unity.manage` 时调用既有 Editor refresh。

`unity.hub.open` 仍仅允许认证 local-owner 且同时具备 `settings.read` / `unity.hub.open`，以准确 revision/
effective path guard 启动；不保存设置、创建 Operation 或推进 revision。download_required 只打开固定官方页面，
未知启动结果不自动重试。其请求、错误与平台行为以既有 RPC/Hub 合同为准。

当前机器合同为 [settings-v4.schema.json](settings-v4.schema.json)，RPC 投影见
[m7-official-gui.schema.json](../rpc/m7-official-gui.schema.json)，窄实例与迁移向量见
[settings-v4.contract-vectors.json](settings-v4.contract-vectors.json)。历史格式分别为
[Config1](settings-v1.md)、[Config2](settings-v2.md)、[Config3](settings-v3.md)，均不再作为当前公开模型。
