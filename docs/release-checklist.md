# P0 候选包验收记录

此清单对应 `candidate.yml` 的 CI Artifacts。P0 只生成未签名候选包，不创建 GitHub Release，
也不把 OCR 模型打进安装包。每次候选构建都应填写 commit、包名、SHA-256 和实际验收环境。

## 构建契约

- [ ] 版本：
- [ ] Commit：
- [ ] `com.azusalens.AzusaLens` 在所有 manifest、macOS Bundle ID、Linux `.desktop` 中一致
- [ ] Windows MSI：`azusa-lens-<version>-x86_64-pc-windows-msvc.msi`
- [ ] macOS Intel DMG：`azusa-lens-<version>-x86_64-apple-darwin.dmg`
- [ ] macOS Apple Silicon DMG：`azusa-lens-<version>-aarch64-apple-darwin.dmg`
- [ ] Arch 包：`azusa-lens-<version>-x86_64.pkg.tar.zst`
- [ ] Linux AppImage：`azusa-lens-<version>-x86_64.AppImage`
- [ ] 所有包均有 `manifest.json` 和 SHA-256 文件
- [ ] 未包含 PP-OCRv6 Tiny、GLM-OCR 或 DeepSeek-OCR 模型文件

## 自动化门禁

- [ ] `quality`：fmt、Clippy、`cargo test --workspace`、桌面构建、许可证检查
- [ ] `native-overlay-linux`：Xvfb 下框选、Enter/Esc、工具栏、属性弹窗、撤销重做和 OCR 选择
- [ ] `ocr-smoke`：固定 PP-OCRv6 Tiny 上游、缓存命中/失效、真实文本块和边界校验
- [ ] `package-smoke`：包内二进制、图标、桌面元数据、版本和应用标识
- [ ] 失败截图已作为 CI Artifact 保存

## 人工验收矩阵

| 平台 | 安装/启动 | PrtSc 或托盘截图 | 框选/标注 | 复制 | 保存 PNG | Tiny OCR/复制文本 | 环境与备注 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Windows x86_64 | [ ] | [ ] | [ ] | [ ] | [ ] | [ ] | |
| macOS x86_64 | [ ] | [ ] | [ ] | [ ] | [ ] | [ ] | 屏幕录制权限 |
| macOS arm64 | [ ] | [ ] | [ ] | [ ] | [ ] | [ ] | 屏幕录制权限 |
| Linux X11 | [ ] | [ ] | [ ] | [ ] | [ ] | [ ] | 托盘、快捷键 |
| KDE Wayland | [ ] | [ ] | [ ] | [ ] | [ ] | [ ] | Portal/Spectacle、权限 |
| GNOME Wayland | [ ] | [ ] | [ ] | [ ] | [ ] | [ ] | 快捷键不可用时须明确提示 |

## 阻断判定

- [ ] 启动、截图、编辑、复制、保存或 OCR 失败时没有丢失当前会话
- [ ] Portal、Spectacle、OCR worker 或模型下载不会无限等待
- [ ] 快捷键只有拿到系统实际绑定后才显示 Active
- [ ] 没有签名/公证提示之外的 P0 阻断问题
- [ ] 已知问题、复现步骤和对应 issue：
