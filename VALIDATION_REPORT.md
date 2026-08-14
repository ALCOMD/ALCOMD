# 初始化包验证报告

生成日期：2026-08-14

## 已通过

- 全部 TOML、JSON 与 YAML 文件语法解析。
- 全部 JSON Schema 自身有效性检查。
- 第一方与示例扩展清单符合 `manifest-v1.schema.json`。
- Cargo Workspace 成员、包名、源入口与路径依赖结构检查。
- npm Workspace 成员与内部包引用结构检查。
- `alcomd.product.toml`、功能清单与稳定身份元数据检查。
- 生产目录旧内部标识扫描。
- Bash 脚本语法检查。
- 文本 UTF-8、尾随空白与文件结尾检查。
- 临时 Git 仓库的 `git diff --cached --check`。
- 内部 TypeScript SDK 包类型检查。
- 全部 TypeScript/TSX 文件语法转译检查。

## 需要在目标开发机执行

当前打包环境没有 Rust 工具链，也不能联网解析 Node/Cargo 依赖，因此以下验证必须在 Windows 开发机或 CI 中完成：

```powershell
.\scripts\init-repo.ps1
.\scripts\setup.ps1
.\scripts\check.ps1 -SkipGuiRust
cargo check -p alcomd-gui
npm run gui:build
```

首次成功验证后，应提交生成的：

```text
Cargo.lock
package-lock.json
```

随后将 CI 中的 `npm install` 切换为 `npm ci`。
