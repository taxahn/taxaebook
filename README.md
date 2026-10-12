# taxaebook

自用的 Rust 电子书 CLI 工具集。目前包含两个功能，共用一个 `taxaebook` 命令：

| 命令 | 功能 | 状态 |
| --- | --- | --- |
| `taxaebook cover` | 从 EPUB、目录或压缩包提取原始封面图片 | 可用 |
| `taxaebook translate` | 按章节切 PDF、调用 MinerU OCR、使用 DeepSeek 翻译 Markdown | Demo |

## 安装

安装 Rust stable，在项目目录执行：

```sh
cargo install --path . --locked
```

安装后可以直接使用 `taxaebook`。若只想在项目内编译：

```sh
cargo build --release --locked
./target/release/taxaebook --help
```

下文以安装后的命令为例；未安装时，将 `taxaebook` 换成 `./target/release/taxaebook`。Windows 的可执行文件为 `target/release/taxaebook.exe`。

也可以从 GitHub Actions 对应运行的 Artifacts 下载构建产物。工作流配置了 Windows x64、Linux x64/ARM64、macOS Intel/Apple Silicon 的原生构建；Linux 基于 Ubuntu 24.04，需要兼容的 glibc。工作流不自动发布 Release，各平台是否通过以实际 CI 结果为准。

## 封面提取

```sh
taxaebook cover book.epub
taxaebook cover first.epub second.epub -o covers
taxaebook cover ./books archive.zip -o covers
```

默认输出到 `covers/`，保持图片原始字节和格式。同名自动编号，不覆盖已有文件。目录递归扫描，压缩包支持 ZIP、TAR、TAR.GZ、TGZ。

支持格式、文件命名和失败行为见 [封面提取文档](docs/cover.md)。

## AI 辅助翻译

OCR 和翻译分别运行，调整术语或重试翻译时可以复用 OCR 结果。先按照 [API 配置说明](docs/translation.md#api-配置) 设置 MinerU Token 和 DeepSeek Key。

可以先用项目中的小段 Markdown 测试翻译，这一步只需要 DeepSeek Key：

```sh
taxaebook translate markdown examples/translation/source.md \
  -o dist/demo/sample \
  --glossary examples/translation/glossary.json --review
```

从 PDF 开始则分三步：

```sh
taxaebook translate prepare book.pdf -o dist/demo/prepared
taxaebook translate ocr dist/demo/prepared -o dist/demo/ocr
taxaebook translate markdown dist/demo/ocr/source.md -o dist/demo/zh --review
```

`prepare` 默认按第 1 层 PDF 书签切章；无书签时用 `--chapters` 提供章节表，或用 `--whole` 显式处理整本小 PDF。新任务的输出目录必须不存在。最终译文为 `dist/demo/zh/translated.md`。

`--review` 启用逐块审校，初稿与审校结果都会保存。中断后沿用原命令和输出目录，加上 `--resume` 可复用已保存响应继续执行。翻译使用 DeepSeek V4.1 Flash，API 模型名为 `deepseek-flash`。新术语先保存为建议，可检查后合并到长期术语库。

完整流程、章节表、术语库、参数与当前限制见 [辅助翻译文档](docs/translation.md)。

## 项目结构

```text
src/
  main.rs                  总入口，分发 cover / translate
  cover/
    cli.rs                 封面参数、目录扫描、去重和批量汇总
    input.rs               EPUB 与压缩包读取
    extract.rs             封面提取、命名和保存
  translation/
    cli.rs                 翻译流程的子命令和参数
    pdf.rs                 书签、章节表和 PDF 切分
    ocr.rs                 MinerU API、原始结果和 Markdown 合并
    llm.rs                 DeepSeek 翻译、上下文和审校
    markdown.rs            Markdown 分块与链接重定位
    glossary.rs            可复用术语库
    mod.rs                 模块入口及内部文件、HTTP 辅助函数
tests/
  cli.rs                   总入口和命令帮助测试
  cover_cli.rs             封面端到端测试
  translation_cli.rs       翻译端到端测试（本地模拟 API）
examples/translation/      Markdown、术语库和章节表示例
docs/                      两个功能的详细使用文档
```

两个功能分别封装在自己的模块中，总入口只负责命令分发。封面解析使用 epub，PDF 处理使用 lopdf，Markdown 处理使用 pulldown-cmark，HTTP 使用 reqwest。

## 开发与验证

```sh
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo build --release --locked
```

无需安装即可运行：

```sh
cargo run -- cover book.epub
cargo run -- translate --help
```

自动测试动态生成 EPUB、压缩包和 PDF，用本机回环 HTTP 服务模拟 MinerU、DeepSeek，不依赖真实 API、密钥或本地样书。本地样书放在被 Git 忽略的 `tests/fixtures/local/`；生成的封面放在 `covers/`，翻译实验结果建议放在 `dist/`，均不提交到 Git。
