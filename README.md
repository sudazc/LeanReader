# LeanReader

LeanReader 是一款 Windows 优先的轻量本地 Markdown 阅读器。支持同窗多标签、创建 Markdown 文件和可选源码编辑；打开已有文档仍默认进入阅读模式。

## 使用

- 点击“打开文件”或按 `Ctrl+O`，可一次选择多篇 `.md` / `.markdown` 文档。
- 点击“新建文件”或按 `Ctrl+N`，创建未命名 Markdown 文档；首次保存时选择文件名和位置。
- 点击顶部标签切换文档；再次打开同一路径时会切回已有标签。
- 可从资源管理器拖入一个或多个 Markdown 文件；标签可拖动排序，右键可关闭当前标签、关闭其他标签或复制路径。
- 左侧目录可折叠；滚动正文时目录会同步标出当前章节。
- 支持 `$...$`、`$$...$$`、`\(...\)`、`\[...\]` 和 `math` 代码块公式；`mermaid` 代码块显示为图表，并可展开源码。公式和图表渲染器只在文档用到时加载。
- 按 `Ctrl+F` 查找当前文档；`Enter` / `Shift+Enter` 切换匹配项，`Esc` 关闭查找。
- 从“文件”菜单打印当前文档或按 `Ctrl+P`；打印内容只包含正文，不包含标签、工具栏和目录。
- 从“文件”菜单导出单文件 HTML；图片、公式样式和字体均内嵌，目录标题锚点保留，保存后可离线打开。
- 点击工具栏“编辑”进入源码与预览并排模式；可拖动分隔条调节宽度，两侧滚动按比例同步。预览在停止输入后更新，行号常显；“查看”菜单可切换自动换行并设置 2、4 或 8 空格缩进。编辑器按需加载，阅读模式不启动编辑器；文件菜单和 `Ctrl+S` / `Ctrl+Shift+S` 在阅读与编辑模式均可保存或另存为。
- 关闭有未保存修改的标签或窗口时，可保存、放弃修改或取消。文件被外部修改或删除时会提示；保存检测到版本冲突时不会覆盖磁盘文件。
- 重启后恢复上次打开的标签、活动文档和各文档阅读位置。会话只在本机保存文件路径和阅读位置，不保存文档正文。
- 正文字号默认 20 px，可调为 15–25 px；在阅读区按住 `Ctrl` 滚动鼠标滚轮，或使用 `Ctrl++` / `Ctrl+-` 调整，按 `Ctrl+0` 恢复默认。界面常驻文字至少 14 px，主要操作文字为 15 px。可切换浅色/深色主题，最近文件和阅读偏好只保存在本机。
- Markdown 相对图片和相对 `.md` / `.markdown` 链接可在本地使用；外部链接仅在点击后交给系统打开。

保存以原始 Markdown 为准，支持保留 UTF-8、UTF-8 BOM、UTF-16 BOM 或 GBK 编码及常见换行格式；GBK 无法表示新增字符时会提示转为 UTF-8。暂不提供所见即所得编辑、直接 PDF 导出、文件关联或云同步。

## 快捷键

| 快捷键 | 操作 |
|---|---|
| `Ctrl+O` | 打开一个或多个 Markdown 文件 |
| `Ctrl+N` | 新建 Markdown 文件 |
| `Ctrl+F` | 查找当前文档 |
| `Enter` / `Shift+Enter` | 下一个 / 上一个匹配项 |
| `Ctrl+S` | 保存当前文档 |
| `Ctrl+Shift+S` | 另存为 |
| `Ctrl+P` | 打印当前文档 |
| `Ctrl+W` | 关闭当前标签 |
| `Ctrl+Tab` / `Ctrl+Shift+Tab` | 切换下一个 / 上一个标签 |
| `Ctrl++` / `Ctrl+-` | 增大 / 减小正文字号 |
| `Ctrl+0` | 恢复默认字号 |
| `Ctrl` + 鼠标滚轮 | 调整正文字号 |
| `Esc` | 关闭查找框或菜单 |

## Windows 安装包

构建 Windows x64 NSIS 安装包：

    npx tauri build --bundles nsis

默认安装包输出到 `src-tauri/target/release/bundle/nsis/`，默认安装到当前用户的 `%LOCALAPPDATA%\LeanReader`。应用使用 Windows WebView2 Runtime；如果运行时缺失，安装程序会从 Microsoft 下载并安装，需联网。无法联网时，请先单独安装 Microsoft Edge WebView2 Runtime。

## 技术

- Tauri 2、Rust
- Svelte 5、SvelteKit、TypeScript、Vite
- markdown-it、highlight.js、DOMPurify
- CodeMirror 6（编辑模式按需加载）
- Windows WebView2

## 开发

安装 Node.js、Rust 工具链和 WebView2 运行环境后，运行：

    npm install
    npx tauri dev

运行前端类型检查和 Rust 单元测试：

    npm run check
    cargo test --manifest-path src-tauri/Cargo.toml --lib
