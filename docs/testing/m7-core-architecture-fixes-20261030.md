# M7 H2-A：Core 架构审查修正

项目所有者明确批准处理只读审查的三项。唯一工作区 main，基线
`7d938cf2845cd982c25b12f106a8dac33981c8e7`，保留此前所有 WIP，未提交／推送。

## 当前实现

1. 设置 PATCH 中 appearance／packages 容器、locale、mode／scheme／density／motion 和三个包设置
   的显式 null 均由协议 DTO 整次拒绝，源字段省略继续表示 unchanged；只有 sourceColor:null
   表示 Clear。原 Schema 本已禁止这些 null，没有增加或放宽合同、版本、依赖或权限。
   真实 RPC 测试复用一个隔离 daemon，10 项混合合法值与非法 null 请求逐项检查 invalid_request、
   配置文件字节和完整设置／revision 不变，随后证明 sourceColor:null 仍真实清除并持久化。
2. application 的 ConfigStore port 使用强类型 ConfigSnapshot，Arc 注入 initialize／load／publish；
   权限、最新读取、CAS、checked revision 加一、窄更新校验和候选内存缓存留在用例层。
   FileConfigStore 由 daemon 组合，文件路径／TOML／Config1/2→3／16 KiB 有界读取／规范序列化／
   同目录 .new/.bak 发布及恢复归 alcomd-store。旧 Path 构造和 application 文件实现已移除，
   不保留双入口或兼容别名。owned guard 持续到 blocking task 发布与缓存完成；调用取消不释放
   中途锁，发布结果不明清空缓存，候选查询仍只读已确认内存快照，不新增文件访问。
3. candidate 和 Unity migration 共享借用式 ProjectAuthorityFingerprint，显式列出 ProjectRecord
   及 ProjectObservation 字段，取代复制整个记录后清除 last_modified 的方式。字段名、顺序和
   序列化字节保持；favorite／registered_at 等现有参与字段继续参与。只保留既有活动时间排除
   策略，不更改 hash 版本、旧 hash、cursor、Plan 重验、resolver、来源或恢复证据。

配色算法继续留在共享 UI；Core 保存选择而不保存派生色板。没有 GUI 直写、第二 writer、
新 Schema／迁移、公共方法／capability／permission、Cargo 或 npm 依赖变化。

## 验证边界

证据目录：`target/m7-core-architecture-fixes-20261030/`。
原配置模块 13 项 parser／持久化／迁移／恢复等测试随实现迁移到 store，未删除或放宽。
新增 application 五项内存 port 测试覆盖权限先于 IO、并发 CAS 只发布一次、非法值／溢出不发布、
未知发布结果失效缓存及确认读取、调用取消后的锁和缓存一致性。
候选冻结 JSON 覆盖 Unicode／数组／可选值、14 个原权威字段变化及活动时间三值不影响；
迁移完整冻结 JSON 与 SHA-256 为 `c3fd587292c757f21ab0faf1f4c012522b5bc717864eb9412d4c9a9cc254c8bb`。

## 当前验证结果

- Application／Store／VPM 完整 lib 回归 **159 passed／0 failed／0 ignored**：33／18／108，
  `core-unit.log`，退出码 0。含前述配置、候选、resolver、事务和恢复的单元测试。
- Protocol Schema 合同 **29/29**，退出码 0；候选快照竞态 **3/3**，真实用户环境亦通过。
- 全 Workspace Clippy `--all-targets -- -D warnings`、最终 fmt、xtask、metadata、baseline freeze
  及 diff check 通过。初次 fmt 发现本轮修改的 store/lib.rs 行尾，已定点统一 Unix 行尾。
- 初次 IPC 测试新增辅助代码错误响应 ID 为 Option<String>，断言误用字符串导致编译失败；
  改为精确 Some(id) 比较后重新编译通过，未改产品响应或放宽断言。
- 真实 settings RPC 已在最终 Workspace 中通过：复用同一隔离 daemon，10 项非法 null 混合
  请求均 invalid_request，文件字节／完整设置／revision 不变；sourceColor:null 正常清除并持久化。
- 最终 `cargo test --workspace --no-fail-fast -- --test-threads=1` **443 passed／0 failed／8 ignored**，
  退出码 0，76 个 runner 与结果组一一对应，避免将 subprocess 输出重复计数。8 个 ignored 均为
  既有 kill/restart 父测试调用的 subprocess 入口，没有新增跳过、放宽断言或修改测试端点。
- 初轮沙箱连接失败，真实用户环境因固定每用户 IPC 被 dev 占用报 AlreadyRunning；这是当时的
  验证环境阻碍，不作为功能失败或通过证据。项目所有者已明确批准暂时停止并恢复原隔离 daemon；
  重新确认 PID／可执行路径／参数后仅停止 PID 59348，保持 GUI PID 56208 打开，再执行完整回归。
- 最终必要构建仅 `cargo build -p alcomd --bin alcomd`，退出码 0，profile 为 dev/debug
  （unoptimized + debuginfo），不是 release。编译日志保留本机 incremental hard-link 回退复制提示。
- daemon 已按原 `target/debug/alcomd.exe --data-dir target/m7-icon-acceptance-20260906/data` 恢复，
  新 PID **60832**。SHA-256：`3553CC79E2419AB657072D01D1EAC01CF117C02A00B9814C3BF5D78C8B6DA5AD`。
  记录在 restored-daemon.json；只读 CLI 禁止自动启动，system status 返回 ready、projects list 成功，
  进程实际路径／参数／二进制摘要一致。GUI PID 56208 的原窗口 ALCOMD3 仍响应；配置文件前后
  SHA-256 不变。见 restoration-check.json、restored-status.json、restored-projects.json。

本轮没有因架构验证改动前端产品代码，未关闭／重建 Tauri 窗口，也未操作真实 Unity 或工程。
只有经明确批准的隔离 daemon 临时停启并升级到本轮构建；配置和隔离目录保留。未取得新增桌面／屏幕阅读器证据；
前次两项列表长文本行高缺口不在本次修正范围，Visual Gate 2 仍 PENDING，不关闭 H2-A。
