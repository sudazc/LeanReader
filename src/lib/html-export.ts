type StaticHtmlOptions = {
  title: string;
  bodyHtml: string;
  darkMode: boolean;
  fontSize: number;
  katexCss: string;
};

const lightTheme = `
  color-scheme: light;
  --surface: #ffffff;
  --surface-muted: #f8f9fb;
  --text: #28313f;
  --muted: #596575;
  --line: #e4e8ed;
  --line-strong: #d7dde5;
  --accent: #426ab8;
  --error: #a52229;
  --code-bg: #f3f5f8;
  --code-comment: #596575;
  --code-keyword: #8255b5;
  --code-string: #2b704f;
  --code-number: #a14f27;
  --code-title: #286da5;
`;

const darkTheme = `
  color-scheme: dark;
  --surface: #20242a;
  --surface-muted: #252a31;
  --text: #e7eaf0;
  --muted: #b8c1cc;
  --line: #30363f;
  --line-strong: #3b434e;
  --accent: #8aa9ee;
  --error: #ff9095;
  --code-bg: #252a31;
  --code-comment: #b8c1cc;
  --code-keyword: #c5a4eb;
  --code-string: #76c895;
  --code-number: #f0a677;
  --code-title: #80b8e9;
`;

const readerStyles = `
  :root { ${lightTheme} }
  :root[data-theme="dark"] { ${darkTheme} }
  * { box-sizing: border-box; }
  html, body { min-height: 100%; margin: 0; background: var(--surface); color: var(--text); }
  body { font-family: "Segoe UI Variable", "Segoe UI", "Microsoft YaHei UI", sans-serif; }
  .reader-content {
    width: min(100%, 780px);
    min-height: 100vh;
    margin: 0 auto;
    padding: 52px 48px 100px;
    color: var(--text);
    font-size: 20px;
    font-weight: 450;
    line-height: 1.8;
    overflow-wrap: anywhere;
    user-select: text;
  }
  .reader-content h1, .reader-content h2, .reader-content h3,
  .reader-content h4, .reader-content h5, .reader-content h6 {
    color: var(--text); font-weight: 700; line-height: 1.35; letter-spacing: -0.025em;
    scroll-margin-top: 24px;
  }
  .reader-content h1 { margin: 0 0 30px; font-size: 1.8em; letter-spacing: -0.04em; }
  .reader-content h2 { margin: 2.15em 0 0.8em; padding-bottom: 0.35em; border-bottom: 1px solid var(--line); font-size: 1.33em; }
  .reader-content h3 { margin: 1.8em 0 0.65em; font-size: 1.11em; }
  .reader-content h4, .reader-content h5, .reader-content h6 { margin: 1.5em 0 0.6em; font-size: 1em; }
  .reader-content p { margin: 0 0 1.15em; }
  .reader-content a { color: var(--accent); text-underline-offset: 3px; }
  .reader-content blockquote { margin: 1.25em 0; padding: 0.12em 0 0.12em 1.05em; border-left: 3px solid var(--line-strong); color: var(--muted); }
  .reader-content blockquote > :last-child { margin-bottom: 0; }
  .reader-content ul, .reader-content ol { margin: 0 0 1.2em; padding-left: 1.55em; }
  .reader-content li { padding-left: 0.2em; }
  .reader-content li + li { margin-top: 0.22em; }
  .reader-content hr { height: 1px; margin: 2.5em 0; border: 0; background: var(--line); }
  .reader-content code { padding: 0.16em 0.38em; border-radius: 4px; color: var(--text); background: var(--code-bg); font-family: "Cascadia Code", Consolas, monospace; font-size: 0.92em; }
  .reader-content pre { margin: 1.35em 0 1.6em; padding: 17px 19px; overflow: auto; border: 1px solid var(--line); border-radius: 8px; background: var(--code-bg); line-height: 1.65; }
  .reader-content pre code { display: block; padding: 0; color: inherit; background: transparent; font-size: 0.9em; white-space: pre; }
  .reader-content table { width: 100%; border-collapse: collapse; margin: 1.4em 0; font-size: 0.92em; }
  .reader-content th, .reader-content td { padding: 9px 12px; border: 1px solid var(--line-strong); text-align: left; vertical-align: top; }
  .reader-content th { background: var(--code-bg); font-weight: 650; }
  .reader-content img { display: block; max-width: 100%; height: auto; margin: 1.3em auto; border-radius: 5px; }
  .reader-content .katex-display { margin: 1.2em 0; overflow-x: auto; overflow-y: hidden; padding: 0.12em 0; }
  .reader-content .katex-error { color: var(--error); white-space: normal; }
  .reader-content .math-block { margin: 1.2em 0; }
  .reader-content .mermaid-block { min-width: 0; margin: 1.45em 0 1.7em; overflow-x: auto; text-align: center; }
  .reader-content .mermaid-block > svg { display: block; width: auto; max-width: 100%; max-height: 720px; height: auto; margin: 0 auto; }
  .reader-content .mermaid-source { margin-top: 0.65em; color: var(--muted); font-size: 14px; text-align: left; }
  .reader-content .mermaid-source summary { width: fit-content; cursor: pointer; font-weight: 550; }
  .reader-content .mermaid-source pre { margin-top: 0.6em; }
  .reader-content .hljs-comment, .reader-content .hljs-quote { color: var(--code-comment); font-style: italic; }
  .reader-content .hljs-keyword, .reader-content .hljs-selector-tag, .reader-content .hljs-literal, .reader-content .hljs-name { color: var(--code-keyword); }
  .reader-content .hljs-string, .reader-content .hljs-regexp, .reader-content .hljs-addition { color: var(--code-string); }
  .reader-content .hljs-number, .reader-content .hljs-attr, .reader-content .hljs-variable, .reader-content .hljs-template-variable { color: var(--code-number); }
  .reader-content .hljs-title, .reader-content .hljs-section, .reader-content .hljs-built_in { color: var(--code-title); }
  .reader-content mark.reader-search-hit { padding: 0 1px; border-radius: 2px; color: #312a1c; background: #fff1bc; }
  .task-list-item { list-style: none; }
  .task-list-item-checkbox { margin: 0 0.55em 0 -1.5em; }
  @media (max-width: 700px) {
    .reader-content { padding: 32px 20px 64px; }
  }
  @media print {
    @page { margin: 18mm; }
    :root, :root[data-theme="dark"] {
      color-scheme: light;
      --surface: #ffffff;
      --surface-muted: #f8f9fb;
      --text: #20242a;
      --muted: #424b57;
      --line: #d7dde5;
      --line-strong: #bdc5cf;
      --accent: #263f70;
      --error: #8e1720;
      --code-bg: #f3f5f8;
      --code-comment: #4d5866;
      --code-keyword: #63418e;
      --code-string: #205c3f;
      --code-number: #87411f;
      --code-title: #205984;
    }
    body { background: #ffffff !important; color: #20242a !important; }
    .reader-content { width: auto; min-height: 0; max-width: none; padding: 0; color: #20242a; }
    .reader-content a { color: #20242a; text-decoration: underline; }
    .reader-content pre { overflow: visible; white-space: pre-wrap; overflow-wrap: anywhere; }
    .reader-content pre, .reader-content table, .reader-content img, .reader-content .mermaid-block { break-inside: avoid; }
    .reader-content h1, .reader-content h2, .reader-content h3, .reader-content h4 { break-after: avoid-page; }
    .reader-content p, .reader-content li { orphans: 3; widows: 3; }
  }
`;

function escapeHtml(value: string): string {
  return value.replace(/[&<>"']/g, (character) => {
    switch (character) {
      case "&": return "&amp;";
      case "<": return "&lt;";
      case ">": return "&gt;";
      case '"': return "&quot;";
      default: return "&#39;";
    }
  });
}

function fontMimeType(url: URL): string {
  if (url.pathname.endsWith(".woff2")) return "font/woff2";
  if (url.pathname.endsWith(".woff")) return "font/woff";
  if (url.pathname.endsWith(".ttf")) return "font/ttf";
  if (url.pathname.endsWith(".otf")) return "font/otf";
  return "application/octet-stream";
}

async function fetchFontDataUrl(url: URL): Promise<string> {
  const response = await fetch(url);
  if (!response.ok) throw new Error("无法读取公式字体文件");
  const bytes = new Uint8Array(await response.arrayBuffer());
  let binary = "";
  for (let offset = 0; offset < bytes.length; offset += 0x8000) {
    binary += String.fromCharCode(...bytes.subarray(offset, offset + 0x8000));
  }
  return `data:${fontMimeType(url)};base64,${btoa(binary)}`;
}

/** 读取并内嵌 KaTeX 样式及字体，保证导出的公式离线可用 */
export async function loadKatexExportCss(): Promise<string> {
  const hasKatexFonts = (value: string) =>
    /@font-face\s*\{[^}]*font-family\s*:\s*["']?KaTeX_/i.test(value);
  let css = "";
  let stylesheetUrl = new URL(document.baseURI);

  for (const stylesheet of Array.from(document.styleSheets)) {
    let stylesheetCss: string;
    try {
      stylesheetCss = Array.from(stylesheet.cssRules, (rule) => rule.cssText).join("\n");
    } catch {
      continue;
    }
    if (!hasKatexFonts(stylesheetCss)) continue;

    const owner = stylesheet.ownerNode;
    css = owner instanceof HTMLStyleElement ? owner.textContent ?? stylesheetCss : stylesheetCss;
    if (stylesheet.href) stylesheetUrl = new URL(stylesheet.href);
    break;
  }

  if (!css) throw new Error("未找到公式样式文件，请重试导出");
  const fontFaces = Array.from(css.matchAll(/@font-face\s*\{[^}]*\}/gi));
  const fontReferences = fontFaces.map((fontFace) => {
    const srcMatch = /src\s*:\s*([^;}]+);?/i.exec(fontFace[0]);
    if (!srcMatch) throw new Error("公式字体声明无效");
    const sources = Array.from(
      srcMatch[1].matchAll(/url\(\s*(['"]?)(.*?)\1\s*\)\s*format\(\s*(['"]?)(.*?)\3\s*\)/gi),
      (match) => ({ source: match[2].trim(), format: match[4].trim() }),
    );
    const selected =
      sources.find((source) => source.format.toLowerCase() === "woff2") ??
      sources.find((source) => source.format.toLowerCase() === "woff") ??
      sources[0];
    if (!selected) throw new Error("公式字体格式不受支持");
    return { fontFace, srcMatch, selected };
  });
  const urls = [
    ...new Set(
      fontReferences
        .map(({ selected }) => selected.source)
        .filter((source) => source && !source.startsWith("data:")),
    ),
  ];
  const embeddedFonts = new Map<string, string>();
  let nextUrl = 0;

  await Promise.all(Array.from({ length: Math.min(4, urls.length) }, async () => {
    while (nextUrl < urls.length) {
      const source = urls[nextUrl++];
      const fontUrl = new URL(source, stylesheetUrl);
      if (fontUrl.origin !== stylesheetUrl.origin) {
        throw new Error("公式样式包含非本地字体，无法生成离线文件");
      }
      embeddedFonts.set(source, await fetchFontDataUrl(fontUrl));
    }
  }));

  const replacements: Array<{ start: number; end: number; value: string }> = [];
  for (const { fontFace, srcMatch, selected } of fontReferences) {
    const dataUrl = selected.source.startsWith("data:")
      ? selected.source
      : embeddedFonts.get(selected.source);
    if (!dataUrl) throw new Error("无法内嵌公式字体");
    const updatedFontFace = fontFace[0].replace(
      srcMatch[0],
      `src:url("${dataUrl}") format("${selected.format}");`,
    );
    const start = fontFace.index ?? 0;
    replacements.push({ start, end: start + fontFace[0].length, value: updatedFontFace });
  }
  let embeddedCss = css;
  for (const replacement of replacements.reverse()) {
    embeddedCss = embeddedCss.slice(0, replacement.start) + replacement.value + embeddedCss.slice(replacement.end);
  }
  return embeddedCss;
}

/** 构建可离线打开的单文件 HTML */
export function buildStaticHtml(options: StaticHtmlOptions): string {
  const title = escapeHtml(options.title);
  const theme = options.darkMode ? "dark" : "light";
  const size = Math.min(28, Math.max(18, Math.round(options.fontSize)));
  const styles = readerStyles.replace("font-size: 20px;", `font-size: ${size}px;`);

  return `<!doctype html>
<html lang="zh-CN" data-theme="${theme}">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <meta name="generator" content="LeanReader">
  <title>${title}</title>
  <style>${styles}\n${options.katexCss}</style>
</head>
<body>
  <main class="reader-content">${options.bodyHtml}</main>
</body>
</html>`;
}
