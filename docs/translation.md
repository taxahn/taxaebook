# AI 辅助翻译

[返回项目首页](../README.md)

`taxaebook translate` 是辅助翻译 demo，分为 PDF 章节准备、MinerU OCR、DeepSeek Markdown 翻译三个阶段。每个阶段单独保存结果，OCR 完成后可以反复调整术语、分块或审校设置，不必重新识别原书。

## API 配置

在 [MinerU API 管理页](https://mineru.net/apiManage) 创建 Token，在 [DeepSeek 开放平台](https://platform.deepseek.com/api_keys) 创建 API Key 并确保账户有可用额度。然后在运行命令的同一个终端设置环境变量。

macOS / Linux（zsh / bash）：

```sh
export MINERU_API_TOKEN='替换为你的 MinerU Token'
export DEEPSEEK_API_KEY='替换为你的 DeepSeek API Key'
```

Windows PowerShell：

```powershell
$env:MINERU_API_TOKEN = '替换为你的 MinerU Token'
$env:DEEPSEEK_API_KEY = '替换为你的 DeepSeek API Key'
```

程序直接读取环境变量，不自动加载 `.env` 文件。密钥不要提交到 Git。

| 配置 | 用途 | 默认值 |
| --- | --- | --- |
| `MINERU_API_TOKEN` | `ocr` 必需 | 无 |
| `DEEPSEEK_API_KEY` | `markdown` 必需 | 无 |
| `MINERU_API_BASE_URL` | 自有代理或本地模拟服务的 MinerU 基址 | `https://mineru.net/api/v4` |
| `DEEPSEEK_API_BASE_URL` | 自有代理或本地模拟服务的 DeepSeek 基址 | `https://api.deepseek.com` |

正常使用只需设置两个密钥。更改基址时，MinerU 地址需包含 `/api/v4`，DeepSeek 地址按代理要求包含 `/v1` 等前缀，不要加 `/chat/completions`。

OCR 使用 MinerU 精准解析 API v4、`vlm` 模型，流程为申请上传链接 → PUT 本地文件 → 轮询 → 下载完整 ZIP。向签名存储地址上传文件、下载结果时不携带 API Token。[MinerU 接口文档](https://mineru.net/apiManage/docs?openApplyModal=true)

LLM 固定使用 DeepSeek V4.1 Flash，其官方 API 模型名为 **`deepseek-flash`**。[DeepSeek 模型说明](https://api-docs.deepseek.com/news/news260910/)

## 先翻译一小段 Markdown

已有 Markdown 可以直接跳过 PDF 和 OCR，只需要配置 DeepSeek Key：

```sh
taxaebook translate markdown examples/translation/source.md \
  -o dist/demo/sample \
  --glossary examples/translation/glossary.json --review
```

查看 `dist/demo/sample/translated.md`。不加 `--review` 是直接翻译；加上后，每块再调用一次同一模型审校，初稿与审校结果均保存，可用于比较效果。

## 从 PDF 开始

### 1. 按章节准备 PDF

默认使用第 1 层书签作为章节边界：

```sh
taxaebook translate prepare book.pdf -o dist/demo/prepared
```

使用其他书签层级时加 `--bookmark-level 2`。无书签或需要调整章节时，参考 [章节表示例](../examples/translation/chapters.json) 创建自己的 `chapters.json`：

```json
[
  { "title": "前言与第一章", "start_page": 1 },
  { "title": "第二章", "start_page": 11 }
]
```

```sh
taxaebook translate prepare book.pdf --chapters chapters.json -o dist/demo/prepared
```

`start_page` 是从 1 开始的 **PDF 物理页码**，不是印刷页码。起始页须严格递增，每章结束于下一章的前一页，最后一章到书末。首章晚于第 1 页时自动保留“前置页”。同页分章会报错。

整本小 PDF 可以明确作为一章处理：

```sh
taxaebook translate prepare short.pdf --whole -o dist/demo/prepared
```

程序按每章最多 **200 页、200 MB** 检查。超限时需要细化章节表，不会自动按固定页数截断。不支持加密 PDF。

### 2. 调用 MinerU OCR

```sh
taxaebook translate ocr dist/demo/prepared -o dist/demo/ocr --language en
```

源语言代码默认 `en`，中文使用 `--language ch`。每次提交一章，默认每 5 秒轮询，每章最长等待 1800 秒；可通过 `--poll-seconds`、`--timeout-seconds` 调整。

完整 ZIP、JSON 和图片均保留，并将各章 `full.md` 按顺序合并到 `source.md`。合并保留 OCR 的标题层级，用注释标记章节边界和原 PDF 页码；图片与链接按章节目录重定位，避免同名图片冲突。

### 3. 翻译和审校

```sh
taxaebook translate markdown dist/demo/ocr/source.md \
  -o dist/demo/zh --review
```

目标语言默认简体中文，使用 `--to` 可以更改。已有术语库时通过 `--glossary my-glossary.json` 指定。

源文按 Markdown 标题分段，再按顶层块控制大小。每块携带命中的术语和前块最后 1200 字译文作为上下文。表格、列表、代码块不从中间拆开；单块超限时明确报错，需要调整上限或手工整理源文。

| 参数 | 含义 | 默认值 |
| --- | --- | --- |
| `--to` | 目标语言 | `简体中文` |
| `--glossary` | 长期术语库 JSON 路径 | 不加载外部术语库 |
| `--chunk-chars` | 每块源文的 Unicode 字符上限，范围 256–20000 | `6000` |
| `--review` | 每块翻译后调用一次审校 | 关闭 |
| `--resume` | 校验源文、初始术语库和参数，复用当前输出目录中的模型响应继续执行 | 关闭 |

字符数是分块依据，不是精确 token 计数。模型输出截断时会报错并保留响应。

## 长期术语库

术语库是可跨书复用的 JSON 对象，参考 [术语库示例](../examples/translation/glossary.json)：

```json
{
  "neural network": "神经网络",
  "gradient descent": "梯度下降"
}
```

翻译中建议的新术语会供本次后续块复用，单独保存到 `terms.suggested.json`，不会自动写入长期库。检查并编辑建议后，再显式合并：

```sh
taxaebook translate glossary-merge my-glossary.json dist/demo/zh/terms.suggested.json
```

目标术语库可以不存在。相同源术语不区分大小写，相同译名会合并，不同译名会报错且整个合并不写入。原有译名不会被覆盖。翻译时按可读正文匹配术语，忽略大小写、换行和 Markdown 强调标记的差别；不同目标语言应使用不同术语库。

模型建议中无法匹配源文、内容为空或与已有译名冲突的条目，写入 `terms.rejected.json` 供人工检查。它们不会进入当前词库，也不会中断译文生成；原始模型响应仍保留。术语建议的校验不等于对译文实际用词的全面验证。

模型可以不返回术语建议；完整、非空的译文仍是必需的。术语字段格式错误时，原字段写入待审清单，不解析其他术语格式。

## 输出文件

| 阶段 | 文件 | 内容 |
| --- | --- | --- |
| 准备 | `prepared/manifest.json` | 章节标题和原 PDF 页码 |
| 准备 | `prepared/chapters/0001.pdf` | 切好的章节 PDF |
| OCR | `ocr/raw/0001/upload.json` | 上传任务响应，含短期有效的签名链接 |
| OCR | `ocr/raw/0001/status.json` | 最后一次查询响应 |
| OCR | `ocr/raw/0001/result.zip` | MinerU 完整原始结果 |
| OCR | `ocr/raw/0001/result/` | 安全解压后的 Markdown、JSON、图片等 |
| OCR | `ocr/source.md` | 按章节顺序合并的源文 |
| 翻译 | `zh/source.md` | 分块前源文，链接已重定位 |
| 翻译 | `zh/run.json` | 模型、目标语言、分块与审校设置 |
| 翻译 | `zh/glossary.used.json` | 初始术语库快照 |
| 翻译 | `zh/chunks/0001/` | 当前块源文、请求、初稿、模型响应和最终译文 |
| 翻译 | `zh/terms.suggested.json` | 新术语建议 |
| 翻译 | `zh/terms.rejected.json` | 未采用的术语建议、所在块和原因 |
| 翻译 | `zh/translated.md` | 全部块成功后生成的完整译文 |

`chunks/0001/` 内包含 `source.md`、`request.json`、`draft.md`、`draft.response.json`、`translated.md`，启用审校时还有 `review.response.json`。

原始 OCR 结果不由 LLM 改写。译文的相对资源链接转换为源文目录的绝对路径，图片引用原 OCR 文件；移动目录后需相应修改链接。

## 失败处理和限制

新任务的 `-o` / `--output` 目录必须不存在，避免覆盖实验结果。继续 Markdown 翻译时，使用原命令、原输出目录并加上 `--resume`：

```sh
taxaebook translate markdown dist/demo/ocr/source.md \
  -o dist/demo/zh --review --resume
```

续跑要求源文、初始术语库、目标语言、分块和审校设置一致。程序校验已保存的每块请求上下文，复用已有模型响应并重建术语与上下文，仅调用尚无响应的步骤；不会再次请求已保存的响应。源文或参数改变时应创建新任务。

- 单章 OCR 失败会保存 `error.txt`，继续其他章节，最后返回非零；有失败时不生成完整 `source.md`。
- 翻译失败会停止，保留已完成块与已有响应，不生成完整 `translated.md`。
- HTTP 非成功状态、无效 JSON、空译文或输出截断都会报错。术语建议的问题单独记录，显式合并长期术语库时遇到冲突仍会报错。
- Demo 不自动重试。Markdown 翻译中断后可显式 `--resume`；已保存响应仍须通过 JSON、非空译文和截断校验，失败响应不会自动替换。需要重新生成响应时用新输出目录重跑；成功的 OCR 结果可以复用。

退出码：`0` 成功，`1` 处理失败，`2` 命令行参数错误。

尚未实现自动补目录、同页分章、OCR 标题层级修复、PDF 译文排版，以及“大块翻译后统一审查”和“逐块翻译审查”的系统评测。图片、表格、公式和脚注的翻译保真度仍需用样书检查。

查看命令帮助：

```sh
taxaebook translate --help
taxaebook translate prepare --help
taxaebook translate ocr --help
taxaebook translate markdown --help
taxaebook translate glossary-merge --help
```
