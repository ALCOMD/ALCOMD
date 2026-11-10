# 扩展包后缀完整拼写（2026-11-10）

## 实施

按所有者要求，扩展包后缀统一为 `.alcomdextension`。更新产品元数据、原生文件选择器、当前包 Profile/Manifest/签名/安全规范、架构说明、测试计划、相关夹具和 Git 二进制文件规则。现有源码的调整只替换后缀文本，保留此前所有未提交修改；逐文件前后摘要见 target/m7-extension-package-name-20261110/rename-inputs.json。

ZIP 容器、内部文件名、Manifest、签名内容摘要、扩展 API、权限及安装/运行流程保持。现有读取器按内容验包，本轮没有增加后缀拒绝条件、旧格式解析器或迁移。用户现存文件和已安装扩展不重命名。历史基线、M6 验收计划和实际运行产物路径保留当时名称，当前名称由 [M7 计划](../exec-plans/M7-extension-package-full-name.md)、产品元数据和 specs/extensions/ 定义。

## 验证

| 范围 | 结果 | 记录 |
| --- | --- | --- |
| 扩展包验证 | 6 项通过：新后缀临时包的解析/验签、Manifest、恶意路径、碰撞及链接、特殊成员类型和非 UTF-8 成员名 | target/m7-extension-package-name-20261110/package-tests.log |
| 实际安装/运行 | 1 项通过：新后缀签名包经隔离 Core 与真实 extension-host 完成生命周期流程 | target/m7-extension-package-name-20261110/lifecycle.log |
| 界面回归 | 32 项通过：扩展工作区、文件选择器、H4 管理和请求错误；包含完整后缀路径转发及失败/取消保留 | target/m7-extension-package-name-browser.log |
| 静态检查 | 扩展包及 GUI all-targets 严格 Clippy、fmt 通过 | target/m7-extension-package-name-20261110/clippy.log、fmt.log |
| 前端 | 全工作区检查及生产构建通过 | target/m7-extension-package-name-20261110/frontend-build.log |

Core/嵌入式 GUI 独立构建及 xtask 仓库门禁通过，构建前后源码摘要一致；Git diff 检查及新后缀的二进制属性核对通过。原生大包警告和指定独立输出文件的 Cargo 提示保留，不影响结果。未运行全工作区 Rust 或跨平台 Hosted CI，也未人工操作原生系统弹窗；本项不扩大 M7/H7 验收。

## 交付边界

验证只使用隔离临时目录及测试签名包。未操作真实扩展、项目、备份或用户文件，未替换运行实例，未提交、推送或发布。最终独立产物、源码摘要及日志输出目录为 target/m7-extension-package-name-20261110/。
