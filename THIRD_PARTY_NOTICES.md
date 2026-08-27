# Third-party notices

ALCOMD v4 自有代码统一使用 `AGPL-3.0-only`，但第三方依赖与资产继续受其各自
许可证约束，不因与 ALCOMD 一同分发而被重新许可。

当前仓库包含的独立第三方资产：

- GUI 图标：Copyright (c) 2024 lilxyzw、anatawa12 及其他贡献者，采用
  Creative Commons Attribution 4.0 International；完整条款见
  `apps/alcomd-gui/icon-LICENSE`，来源提交、原始摘要与生成文件摘要见
  `docs/baselines/asset-provenance.toml`。
- Official GUI 使用从 Google 官方 `google/material-design-icons` 仓库固定提交
  `e083cc60a0828fdd3b404cea0cb8a5b900e9c23e` vendoring 的 Material Symbols SVG。只包含产品实际使用的
  Rounded / weight 400 / grade 0 / fill 0 / opsz 20 或 24 资产，采用 Apache License 2.0；完整文件清单、
  upstream path 与 SHA-256 见 `packages/alcomd-ui/assets/material-symbols/manifest.toml`。

构建依赖的许可证记录由 Cargo 与 npm lockfile 保留，发行前必须生成完整依赖清单。

ALCOMD v4 未复制、移植或改写 ALCOMD3 v3、vrc-get 或 vrc-get-vpm 的源码。它们
不是本项目的代码上游，也不构成本项目自有代码的第三方组成部分。
