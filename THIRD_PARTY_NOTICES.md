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

共享主题色引擎使用 Google 的 `@material/material-color-utilities 0.4.0`（Apache License 2.0），
来源为 `https://github.com/material-foundation/material-color-utilities`；算法文件的 Google LLC
版权和许可证声明随上游包保留。版本与完整性摘要固定在 `package-lock.json`。
此依赖用于从种子色生成 Material 语义色板，不包含 v3 或 VRCLearn 的主题源码。

构建依赖的许可证记录由 Cargo 与 npm lockfile 保留，发行前必须生成完整依赖清单。

Official GUI 容器自绘滚动条使用 `overlayscrollbars 2.17.0`（MIT），来源为
`https://github.com/KingSora/OverlayScrollbars`，版权及许可原文见该仓库的 `LICENSE`；
版本与完整性摘要固定在 `package-lock.json`，未引入 v3 的滚动条源码。

OverlayScrollbars 2.17.0 license text (upstream tag `v2.17.0`):

```text
MIT License

Copyright (c) 2022 Rene Haas

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:
The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.
THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

ALCOMD v4 未复制、移植或改写 ALCOMD3 v3、vrc-get 或 vrc-get-vpm 的源码。它们
不是本项目的代码上游，也不构成本项目自有代码的第三方组成部分。
