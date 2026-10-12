# AGENTS.md

## 项目目标

taxaebook 是用 Rust 编写的 ebook CLI 工具集，共用一个命令入口。`taxaebook cover` 接收单文件、多文件、目录或支持的压缩包，提取 EPUB 声明的原始封面，不重新编码图片。`taxaebook translate` 将章节切分、MinerU OCR 和 DeepSeek 翻译分阶段运行，保留中间结果，以 Markdown 输出译文。

## 开发原则

1. 不保留向后兼容。过时的直接删，别加兼容层、别写 migration、别留 fallback。
2. 选能满足当前需求的最简单实现。不要预防性抽象，不要多此一举的配置层。
3. 系统分层长。先跑通一个最小的端到端版本，再往上加东西。绝不为了未完成的复杂度拆掉能跑的东西。
4. 组件保持模块化，关注点分离。
5. 优先用成熟的、有人维护的库。没有明确理由别自己重写。
6. 先翻项目里已有的依赖能做什么，再考虑加新包或自己写。别上来就假设库里没有。
7. 架构决策往长了做。不接受“先这样以后再换”的临时方案。
8. 先看成熟产品怎么解决同一个问题，用已验证的模式，别从零发明。

## 模块职责

- `src/main.rs`：总 CLI 入口，只分发 `cover` 和 `translate`。
- `src/cover/cli.rs`：封面 CLI 参数、目录扫描、输入去重、批量执行和结果汇总。
- `src/cover/input.rs`：识别输入格式、遍历压缩包、提供单本 EPUB 的可读取文件。
- `src/cover/extract.rs`：调用 EPUB 解析库获取封面，确定图片扩展名并保存输出。
- `src/translation/cli.rs`：翻译 CLI 参数和 `prepare`、`ocr`、`markdown`、`glossary-merge` 命令分发。
- `src/translation/pdf.rs`：PDF 书签/手工章节表和章节切分。
- `src/translation/ocr.rs`：MinerU 上传、轮询、原始结果保存和源文合并。
- `src/translation/llm.rs`：DeepSeek 分块翻译、上下文、审校和中间结果。
- `src/translation/markdown.rs`、`glossary.rs`：Markdown 分块/链接处理和长期术语库。
- 各功能的 `mod.rs`：对总入口只导出 CLI 参数和执行函数，内部模块不跨功能引用。
- `tests/cli.rs`：总入口、子命令路由和帮助信息测试。
- `tests/cover_cli.rs`：动态 EPUB 和压缩包的封面 CLI 端到端测试。
- `tests/translation_cli.rs`：动态 PDF 和本地模拟 API 的翻译 CLI 端到端测试，不调用真实 API。
- `README.md`：安装、快速上手和项目导航；`docs/cover.md`、`docs/translation.md`：各功能的完整使用说明。
- `.github/workflows/build.yml`：Windows、Linux、macOS 的测试和构建。

修改时在对应模块内完成职责，不把 EPUB 解析、压缩包处理和命令行逻辑堆到一起。复用现有的 epub、zip、tar、flate2、clap、walkdir 和 tempfile 等依赖。

## 行为要求

- 保留封面原始字节；未声明封面或资源无法读取时明确报错，不猜测正文插图作为封面。
- 输出不能覆盖已有文件；封面同名输出自动编号，文件名须适用于 Windows、Linux 和 macOS。新翻译任务要求输出目录不存在，保留中间结果；显式 `--resume` 校验任务一致后复用已有响应。
- 压缩包内部路径不能直接用作磁盘输出路径；临时文件使用后自动清理。
- 批量处理时单本失败不能阻止其他可处理输入；最终返回非零退出码报告失败。
- 标准输出记录保存结果，标准错误记录进度、错误和汇总。新增参数保持 `--help` 清晰。
- 改变输入格式、参数、输出命名或错误行为时同步更新 README、对应使用文档和相关测试。
- 不把本机验证表述为全部平台验证；跨平台结果以对应 CI 实际运行结果为准。

## 验证

Rust 代码变更完成后运行：

```sh
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
```

涉及依赖、构建或发布配置时，再验证 `cargo build --release --locked`。仅文档变更无需重跑 Rust 测试，检查内容与 `git diff --check` 即可。

测试关注用户可观察的行为和失败边界，不编写仅复述实现的测试。自动测试不得依赖本地样书、网络或固定的用户目录。

## 测试文件与 Git

- 本地样书放在 `tests/fixtures/local/`，保持 Git 忽略；不要提交受版权保护的书籍。
- `target/`、`covers/`、`dist/` 等生成文件不提交；提交 `Cargo.lock`，保证 CLI 依赖可复现。
- 主分支为 `main`。提交前检查 diff 和暂存内容，只提交任务相关改动。
- 未经用户要求，不推送远端或发布 Release。
