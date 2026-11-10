# 默认存储根改为用户主目录（2026-11-10）

## 实施范围

按所有者要求，将默认项目及备份根从 Documents/ALCOMD 改为用户主目录/ALCOMD。Windows 复用现有 Known Folder 窄适配器查询 FOLDERID_Profile；macOS/Linux 读取 HOME，不再读取 Documents 或 Linux user-dirs.dirs。移除弃用解析及对应旧用例，产品目录继续由 alcomd.product.toml 派生，平台发现不创建目录。

路径设置和内部状态目录合同保持。显式路径仍优先；新默认只作用于未设置的路径。当前实例两项均为 null，因此 settings.get 的 directories 派生值会更新，持久设置与 revision 不变。旧项目、备份及已接受任务仍使用原位置，不搬迁、不重写定位。没有 RPC/Schema/权限/依赖变化。

## 验证

4 项平台存储测试、2 项 Windows Known Folder/COM 测试、33 项应用设置测试及 1 项存储集成测试通过，共 40 项；覆盖默认根、目录准备、不覆盖、显式覆盖与清空、旧备份/已接受任务目的地保持。记录为 target/m7-home-storage-platform-scoped.log、m7-home-storage-known-folder.log、m7-home-storage-application.log、m7-home-storage-integration.log。平台 all-targets 严格 Clippy、fmt、xtask 及独立 Core/嵌入式 GUI 构建通过。

最初运行完整平台套件时，4 项与此次修改无关的 junction/Hub 子进程用例等待超过 60 秒，已终止该自有测试进程，不计全套通过；普通与真实 Windows 用户进程权限不同，首次清理失败后在原测试用户环境完成，未触及运行 Core/GUI。随后仅在真实 Windows 用户环境完成上述受影响定向验证。Current Config13 向量同步新的派生默认根，历史 Config1–12 迁移输入保持。额外运行旧 scripts/validate-metadata.py 时，因其 CURRENT_CONFIG_SCHEMA 仍为 12 而实际 RPC 已为 13 失败；已核对 HEAD 本来就存在该不一致，不修改或放宽该脚本，也不计为通过，依据在 target/m7-home-storage-update-20261110/baseline-metadata-mismatch.json。

Unix 分支已更新，但本轮执行环境为 Windows，未执行 macOS/Linux 运行验证。

## 运行交付

保留所有此前修改，不操作真实项目或备份、不提交或推送。已离线逐文件备份旧程序、29 份数据文件和 326 份 WebView 配置，并更新打开 Core PID43844、GUI PID2416，ready/State31/Config13、0 活动任务。实际 settings.get 默认值为 C:\Users\M2922\ALCOMD\Projects、C:\Users\M2922\ALCOMD\Backups；Core 已按既有启动流程准备新默认项目目录，备份目录仍在创建备份时按需准备。持久设置字节、完整 settings/revision5、3 项目/30 仓库/5 模板、原 profile 与 v3/Unity 保持，9 份真实项目清单摘要保持，没有迁移任何项目或备份。切换后的唯一预期设置差异是 directories 派生值；GUI 打开后 15 条仓库正常后台刷新时间推进，其余仓库字段保持，差异单独记录。原数据及资源核对通过。输入、产物摘要和可回退备份在 target/m7-home-storage-update-20261110/。未进行原生设置页点击或新建项目/备份操作，前端源码保持此前验证输入。M7/H7 状态保持。
