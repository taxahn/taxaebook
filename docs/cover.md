# 封面提取

[返回项目首页](../README.md)

`taxaebook cover` 提取 EPUB 声明的原始封面，保持图片字节和格式，不重新编码。

## 使用

```sh
taxaebook cover book.epub
taxaebook cover first.epub second.epub -o output
taxaebook cover ./books
taxaebook cover books.zip books.tar books.tar.gz books.tgz -o output
taxaebook cover book.epub ./books archive.zip -o output
taxaebook cover --help
```

路径有空格时加引号。可混合传入单文件、目录和压缩包；目录递归扫描，不跟随目录中的符号链接。扩展名不区分大小写，同一次运行的重复文件路径会去重。

## 支持范围

| 输入 | 行为 |
| --- | --- |
| EPUB 2 | 读取 `cover` 元数据声明的封面 |
| EPUB 3 | 读取 `cover-image` 声明的封面 |
| 目录 | 递归查找支持的文件 |
| ZIP | 支持 Stored / Deflate，读取任意目录下的 EPUB |
| TAR、TAR.GZ、TGZ | 读取任意目录下的 EPUB |

封面格式支持 JPEG、PNG、GIF、WebP、SVG、AVIF、BMP、TIFF。未声明封面、封面资源为空或无法读取时明确报错，不猜测正文插图作为封面。

不递归打开压缩包内的其他压缩包。不支持 RAR、7z、密码保护和 DRM 解密。压缩包中的 EPUB 逐本写入自动清理的临时文件，包内路径不会直接解压到文件系统；封面图片由解析库读入内存。

## 输出

`-o` / `--output` 指定输出目录，默认为 `covers/`。

例如 `book.epub` 中的 JPEG 封面保存为 `cover-book.jpg`。已有同名文件时，自动使用 `cover-book-2.jpg`、`cover-book-3.jpg`，不覆盖原文件。

文件名适用于 Windows、Linux 和 macOS：特殊字符替换为下划线，书名最多保留 60 个字符，并使用 `cover-` 前缀避开 Windows 保留名称。

## 错误和退出码

批量处理中，单本 EPUB 失败后继续处理其他书籍。无法继续解析的压缩包会终止该压缩包的读取，继续处理其他输入。

标准输出记录提取结果和保存路径，标准错误记录错误与成功/失败汇总。

| 退出码 | 含义 |
| --- | --- |
| `0` | 全部成功 |
| `1` | 有处理失败，或未找到 EPUB |
| `2` | 命令行参数错误 |
