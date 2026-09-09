<div align="center">

# Azusa Lens

轻量、跨平台的截图、标注与本地 OCR 工具。

简体中文 · [English](README.md)

[![CI](https://github.com/BruceZhang1993/AzusaLens/actions/workflows/ci.yml/badge.svg)](https://github.com/BruceZhang1993/AzusaLens/actions/workflows/ci.yml)
[![License](https://img.shields.io/github/license/BruceZhang1993/AzusaLens)](LICENSE)
![Rust](https://img.shields.io/badge/Rust-1.98.1-orange?logo=rust)
![Platforms](https://img.shields.io/badge/platform-Windows%20%7C%20macOS%20%7C%20Linux-blue)

<a href="https://slint.dev/"><img src="https://raw.githubusercontent.com/slint-ui/slint/master/logo/MadeWithSlint-logo-whitebg.png" alt="Made with Slint" height="24"></a>

</div>

> Azusa Lens 仍在持续开发中。通过 GitHub Actions 的 `Candidate packages` 手动工作流可以获取未签名候选安装包；正式发布、签名、公证和自动更新尚未纳入当前阶段。

## 功能

- 支持 Windows、macOS、Linux X11 与原生 Wayland Portal 的区域截图
- 全局 `PrtSc` 快捷键与系统托盘工作流
- 矩形、椭圆、箭头、直线、画笔、文本、序号、马赛克、模糊等标注工具
- 支持标注选择、移动、缩放、样式调整、撤销与重做
- 本地 OCR 文本层，可直接选择识别结果，并保留旋转/倾斜文本几何信息
- 内置 PP-OCRv6 本地模型，并可选通过本地 Ollama 使用 GLM-OCR 与 DeepSeek-OCR
- OCR 模型支持下载进度、取消、启用、重试与删除
- 支持亮色、暗色与跟随系统外观
- 支持复制到剪贴板与 PNG 导出

## 使用

1. 启动 Azusa Lens，应用会常驻系统托盘。
2. 按 **PrtSc**，或从托盘选择 **Capture region** 开始截图。
3. 框选区域后，使用悬浮标注工具进行编辑。
4. 使用 OCR 前，进入 **OCR 模型** 页面下载并手动启用模型。应用不会自动下载模型。
5. 完成后可复制编辑结果或保存为 PNG。

主界面可从托盘打开，用于管理外观、快捷键、截图设置、导出设置、OCR 模型和应用信息。

### OCR

PP-OCRv6 模型直接在本机进程内运行。GLM-OCR 与 DeepSeek-OCR 为可选能力，需要本地运行 Ollama。Azusa Lens 不会静默切换到云端 OCR 服务。

自定义 PP-OCR 模型目录：

```bash
AZUSA_LENS_OCR_MODEL_DIR=/path/to/models
```

如果文本标注无法自动找到合适的系统字体，可指定字体文件：

```bash
AZUSA_LENS_FONT=/path/to/font.ttf
```

模型存储、运行时、来源与隐私说明请查看 [OCR 模型文档](docs/ocr-models.md)。

## 构建与运行

### 环境要求

- Rust **1.98.1**
- Git
- 仅在使用 GLM-OCR 或 DeepSeek-OCR 时需要 Ollama

仓库已提供 `rust-toolchain.toml`，使用 `rustup` 时会自动选择项目需要的 Rust 工具链。

克隆并运行：

```bash
git clone https://github.com/BruceZhang1993/AzusaLens.git
cd AzusaLens
cargo run -p azusa-lens-desktop
```

### Linux 依赖

Debian / Ubuntu：

```bash
sudo apt-get update
sudo apt-get install -y --no-install-recommends \
  pkg-config libclang-dev libxcb1-dev libxrandr-dev libdbus-1-dev \
  libpipewire-0.3-dev libwayland-dev libegl-dev libgbm-dev libx11-xcb-dev \
  libxcursor-dev libxkbcommon-x11-dev libxkbcommon-dev libx11-dev \
  libxcb-shape0-dev libxcb-xfixes0-dev libfontconfig-dev
```

macOS 首次截图可能需要授予 **屏幕录制** 权限。Wayland 下的截图和全局快捷键能力取决于桌面环境及 Portal 实现。

## 开发调试

常用检查命令：

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo check -p azusa-lens-desktop
```

VS Code 用户安装仓库推荐扩展后，可以在 Run and Debug 面板中直接运行 **Debug Azusa Lens**。

### 项目结构

```text
apps/desktop       桌面应用与 Slint UI
crates/core        公共领域模型
crates/capture     跨平台截图能力
crates/hotkey      全局快捷键抽象
crates/annotation  标注文档与渲染
crates/ocr         OCR 引擎与模型管理
```

CI 会执行格式检查、Clippy、全量测试、许可证合规检查、Xvfb 原生 overlay 门禁和 PP-OCRv6 Tiny 真实识别 smoke；通过 `Candidate packages` 手动工作流可构建 Windows MSI、macOS DMG、Arch pacman 包和 Linux AppImage。候选包只作为 CI Artifact 提供，不创建 GitHub Release。

## 参与贡献

欢迎提交 Issue 和 Pull Request。提交 PR 前，建议在本地完成格式检查、Clippy 和测试。

## 开源协议

Azusa Lens 使用 [Apache License 2.0](LICENSE) 开源协议。

Slint 按照 Slint Royalty-Free Desktop, Mobile, and Web Applications License 2.0 使用。第三方许可证及署名说明见 [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md)。
