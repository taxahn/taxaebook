# taxaebook

用 Rust 编写的 EPUB 封面提取 CLI。直接保存书中声明的原始图片，不重新编码。

## 使用

```sh
taxaebook book.epub
taxaebook first.epub second.epub -o output
taxaebook ./books
taxaebook books.zip books.tar books.tar.gz books.tgz -o output
taxaebook book.epub ./books archive.zip -o output
taxaebook --help
```

路径有空格时加引号。文件夹默认递归扫描，不跟随目录中的符号链接；扩展名不区分大小写。同一次运行的重复文件路径会去重。

输出默认保存到当前工作目录的 `covers/`，命名为 `cover-书籍文件名.jpg` 等，保持原始图片格式。同名自动加 `-2`、`-3`，不覆盖已有文件。为适配 Windows，名称中的特殊字符会替换为下划线，书名最多保留 60 个字符。

支持 EPUB 2 的 cover 元数据和 EPUB 3 的 cover-image 声明，图片类型包括 JPEG、PNG、GIF、WebP、SVG、AVIF、BMP、TIFF。未声明封面或资源损坏会报错，不猜测正文插图作为封面。

压缩包支持 ZIP（Stored / Deflate）、TAR、TAR.GZ、TGZ，读取包内任意目录下的 EPUB；不递归打开包内的其他压缩包。RAR、7z、密码保护和 DRM 解密不支持。压缩包中的 EPUB 逐本写入自动清理的临时文件，包内路径不会解压到文件系统；封面图片由解析库读入内存。

批量处理时，单本 EPUB 失败后继续处理其他书籍。无法继续解析的压缩包会终止该压缩包的读取，继续处理其他输入。标准输出记录保存路径，标准错误记录错误和汇总。退出码：`0` 全部成功，`1` 处理失败或未找到 EPUB，`2` 命令行参数错误。

## 编译和安装

安装 Rust stable 后，在项目目录执行：

```sh
cargo build --release --locked
cargo install --path . --locked
```

可执行文件在 `target/release/taxaebook`；Windows 上为 `target/release/taxaebook.exe`。安装后可直接调用 `taxaebook`。

GitHub Actions 在 push、PR 或手动触发时，分别运行测试并生成 Windows x64、Linux x64/ARM64、macOS Intel/Apple Silicon 的可执行文件。在对应运行的 Artifacts 中下载；Linux 构建基于 Ubuntu 24.04（需要兼容的 glibc）。每个平台在对应 runner 原生构建，无须用户安装 Rust 即可运行下载的程序。工作流只上传构建产物，不自动发布 Release。

## 开发和测试

```sh
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo run -- tests/fixtures/local -o covers
```

自动测试在临时目录生成 EPUB 2/3 和压缩包，不依赖真实书籍。用户提供的两本样书移到了 `tests/fixtures/local/`，被 Git 忽略，不随仓库或 CI 分发。

模块：`src/input.rs` 负责输入格式与压缩包遍历，`src/cover.rs` 负责 EPUB 封面与安全保存，`src/main.rs` 负责 CLI 和批量汇总。解析复用 [epub](https://docs.rs/epub/)，压缩包使用 zip、tar、flate2，命令行使用 clap。
