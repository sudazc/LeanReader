<script lang="ts">
  import { onMount, tick } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { confirm, open, save } from "@tauri-apps/plugin-dialog";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import MarkdownEditor from "$lib/MarkdownEditor.svelte";
  import { fadeTransition, panelTransition, popupTransition } from "$lib/popup-transition";
  import {
    BookOpen,
    Bold,
    Check,
    ChevronDown,
    ChevronLeft,
    ChevronRight,
    Code,
    Download,
    Eye,
    EyeOff,
    FileText,
    FolderOpen,
    Italic,
    Link,
    LogOut,
    Minus,
    Moon,
    PanelLeftClose,
    PanelLeftOpen,
    Pencil,
    Plus,
    Printer,
    RotateCcw,
    Search,
    Save,
    Square,
    Sun,
    TextWrap,
    Trash2,
    X,
  } from "lucide-svelte";
  import type { DocumentHeading, RenderedDocument } from "$lib/markdown";
  import type { Compartment, EditorState } from "@codemirror/state";

  type MarkdownDocument = {
    path: string;
    name: string;
    content: string;
    encoding: string;
    version: FileVersion;
  };

  type FileVersion = {
    size: number;
    modifiedNs: string;
  };

  type MarkdownFileStatus = {
    path: string;
    version: FileVersion | null;
  };

  type MarkdownWriteResult = {
    path: string;
    name: string;
    encoding: string;
    version: FileVersion;
  };

  type ReaderTab = {
    id: string;
    editorInstanceId: string;
    editorState: EditorState | null;
    editorCompartments: EditorCompartments | null;
    savedEditorDoc: EditorState["doc"] | null;
    previewedEditorDoc: EditorState["doc"] | null;
    previewedEditorPath: string | null;
    path: string;
    name: string;
    source: string;
    savedSource: string;
    html: string;
    headings: DocumentHeading[];
    scrollTop: number;
    editorScrollTop: number;
    editorScrollProgress: number;
    draftUpdatedAt: number;
    cursor: number;
    encoding: string;
    lineEnding: string;
    version: FileVersion | null;
    externalChange: "modified" | "missing" | null;
    editing: boolean;
    previewGeneration: number;
    livePreviewGeneration: number;
    loadRequestId: string | null;
    loading: boolean;
    loadStarted: boolean;
    error: string | null;
    pendingFragment?: string;
  };

  type RecentFile = {
    path: string;
    name: string;
    openedAt: number;
  };

  type SessionTab = {
    path: string;
    scrollTop: number;
  };

  type ReaderSession = {
    tabs: SessionTab[];
    activePath: string | null;
  };

  type DraftRecoveryRecord = {
    name: string;
    source: string;
    cursor: number;
    editorScrollProgress: number;
    lineEnding: string;
    updatedAt: number;
  };

  type MarkdownModule = typeof import("$lib/markdown");
  type MarkdownImageLoader = (source: string) => Promise<string>;
  type EditorCompartments = {
    lineWrapping: Compartment;
    indentation: Compartment;
  };

  const recentStorageKey = "markdown-reader:recent-files";
  const preferenceStorageKey = "markdown-reader:preferences";
  const sessionStorageKey = "markdown-reader:session";
  const draftRecoveryStorageKey = "markdown-reader:recovery-drafts";
  const maxRecentFiles = 8;
  const defaultFontSize = 20;
  const minFontSize = 15;
  const maxFontSize = 25;
  const defaultEditorIndentWidth = 2;
  const editorIndentWidths = [2, 4, 8] as const;
  const defaultOutlineWidth = 232;
  const minOutlineWidth = 176;
  const maxOutlineWidth = 360;
  const minEditorSplitRatio = 0.25;
  const maxEditorSplitRatio = 0.75;

  let tabs = $state<ReaderTab[]>([]);
  let activeTabId = $state<string | null>(null);
  let activeTab = $derived(tabs.find((tab) => tab.id === activeTabId) ?? null);
  let recentFiles = $state<RecentFile[]>([]);
  let darkMode = $state(false);
  let tocVisible = $state(true);
  let outlineWidth = $state(defaultOutlineWidth);
  let fontSize = $state(defaultFontSize);
  let editorWordWrap = $state(true);
  let editorIndentWidth = $state(defaultEditorIndentWidth);
  let editorSplitRatio = $state(0.5);
  let editorPreviewVisible = $state(false);
  let openMenu = $state<"file" | "recent" | "view" | "insert" | null>(null);
  let searchOpen = $state(false);
  let searchQuery = $state("");
  let searchCount = $state(0);
  let activeSearchIndex = $state(0);
  let activeHeadingId = $state<string | null>(null);
  let notice = $state("");
  let tabContextMenu = $state<{ tabId: string; x: number; y: number } | null>(null);
  let zoomedImage = $state<{ src: string; alt: string } | null>(null);
  let pendingCloseIds = $state<string[]>([]);
  let closeAfterConfirmation = $state(false);
  let closeSaveInProgress = $state(false);
  let allowConfirmedWindowClose = false;
  let closePromptGeneration = 0;
  let editorComponent:
    | {
        scrollToLine(line: number): void;
        scrollToProgress(progress: number): void;
        focusEditor(): void;
        openSearch(): void;
        insertMarkdown(action: "bold" | "italic" | "inlineCode" | "codeBlock" | "link"): void;
      }
    | undefined = $state();
  let tabStripElement = $state<HTMLDivElement>();
  let readerViewport = $state<HTMLDivElement>();
  let readerContent = $state<HTMLElement>();
  let editorSplitElement = $state<HTMLDivElement>();
  let editorSplitResizeBounds: {
    left: number;
    splitterWidth: number;
    availableWidth: number;
  } | null = null;
  let outlineResizeBounds: { left: number; splitterWidth: number } | null = null;
  let searchInput = $state<HTMLInputElement>();
  let lastFontZoomAt = Number.NEGATIVE_INFINITY;
  let noticeTimer: ReturnType<typeof setTimeout> | undefined;
  let sessionSaveTimer: ReturnType<typeof setTimeout> | undefined;
  let draftRecoveryTimer: ReturnType<typeof setTimeout> | undefined;
  let externalCheckTimer: ReturnType<typeof setInterval> | undefined;
  const editorPreviewTimers = new Map<string, ReturnType<typeof setTimeout>>();
  const readerCodeCopyTimers = new WeakMap<HTMLButtonElement, ReturnType<typeof setTimeout>>();
  const externalChecksInFlight = new Set<string>();
  let restoringSession = false;
  let draggedTabId: string | null = null;
  let unlistenNativeEvents: Array<() => void> = [];
  // 同一标签的保存操作串行执行，避免并发写入造成误报冲突
  const saveOperations = new WeakMap<ReaderTab, Promise<boolean>>();
  let markdownModulePromise: Promise<MarkdownModule> | undefined;

  function loadMarkdownModule(): Promise<MarkdownModule> {
    if (markdownModulePromise) return markdownModulePromise;

    const loading = import("$lib/markdown");
    markdownModulePromise = loading;
    void loading.catch(() => {
      if (markdownModulePromise === loading) markdownModulePromise = undefined;
    });
    return loading;
  }

  async function renderMarkdown(
    source: string,
    loadImage: MarkdownImageLoader,
  ): Promise<RenderedDocument> {
    const markdown = await loadMarkdownModule();
    return markdown.renderMarkdown(source, loadImage);
  }

  function loadDocumentImage(documentPath: string): MarkdownImageLoader {
    return (source) => invoke<string>("read_local_image", { documentPath, source });
  }

  async function printActiveDocument() {
    const tab = activeTab;
    if (!tab || tab.loading || tab.error) return;
    openMenu = null;

    const source = sourceForTab(tab);
    let rendered = { html: tab.html, headings: tab.headings };
    if (
      tab.editing &&
      (tab.previewedEditorDoc !== tab.editorState?.doc || tab.previewedEditorPath !== tab.path)
    ) {
      try {
        rendered = await renderMarkdown(source, loadDocumentImage(tab.path));
      } catch (error) {
        showNotice("准备打印失败：" + displayError(error, "请稍后重试。"));
        return;
      }
    }

    const currentTab = tabs.find((item) => item.id === tab.id);
    if (!currentTab || activeTabId !== tab.id || sourceForTab(currentTab) !== source) {
      showNotice("文档内容已变化，请重新打印。");
      return;
    }
    currentTab.html = rendered.html;
    currentTab.headings = rendered.headings;
    tabs = [...tabs];
    await tick();
    if (activeTabId === tab.id) window.print();
  }

  async function exportActiveDocument() {
    const tab = activeTab;
    if (!tab || tab.loading || tab.error) return;
    openMenu = null;

    try {
      const source = sourceForTab(tab);
      const rendered = await renderMarkdown(source, loadDocumentImage(tab.path));
      const htmlExport = await import("$lib/html-export");
      const containsKatex = /class=["'][^"']*\bkatex\b/.test(rendered.html);
      const katexCss = containsKatex ? await htmlExport.loadKatexExportCss() : "";
      const html = htmlExport.buildStaticHtml({
        title: tab.name.replace(/\.(?:md|markdown)$/i, ""),
        bodyHtml: rendered.html,
        darkMode,
        fontSize,
        katexCss,
      });
      const defaultPath = tab.path
        ? tab.path.replace(/\.(?:md|markdown)$/i, ".html")
        : `${tab.name.replace(/\.(?:md|markdown)$/i, "")}.html`;
      const targetPath = await save({
        defaultPath,
        filters: [{ name: "HTML 文档", extensions: ["html"] }],
      });
      if (!targetPath) return;

      await invoke("write_export_html", { path: targetPath, content: html });
      showNotice("HTML 已导出");
    } catch (error) {
      showNotice("HTML 导出失败：" + displayError(error, "请检查文件夹访问权限和磁盘空间。"));
    }
  }

  function pathKey(path: string): string {
    let normalized = path.replaceAll("/", "\\").replace(/\\+$/, "");
    const extendedPrefix = "\\\\?\\";
    if (normalized.toLowerCase().startsWith(extendedPrefix)) {
      normalized = normalized.slice(extendedPrefix.length);
      if (/^unc\\/i.test(normalized)) normalized = "\\\\" + normalized.slice(4);
    }
    return normalized.toLowerCase();
  }

  function displayPath(path: string): string {
    const extendedPrefix = "\\\\?\\";
    if (!path.toLowerCase().startsWith(extendedPrefix)) return path;

    const standardPath = path.slice(extendedPrefix.length);
    if (/^unc[\\/]/i.test(standardPath)) return "\\\\" + standardPath.slice(4);
    if (/^[a-z]:[\\/]/i.test(standardPath)) return standardPath;
    return path;
  }

  function nameFromPath(path: string): string {
    return path.split(/[\\/]/).filter(Boolean).at(-1) || "Markdown 文档";
  }

  function scrollBehavior(): ScrollBehavior {
    return window.matchMedia("(prefers-reduced-motion: reduce)").matches
      ? "auto"
      : "smooth";
  }

  function showNotice(message: string) {
    notice = message;
    if (noticeTimer) clearTimeout(noticeTimer);
    noticeTimer = setTimeout(() => (notice = ""), 3600);
  }

  function displayError(error: unknown, fallback: string) {
    return typeof error === "string" && error.trim() ? error : fallback;
  }

  function saveRecentFiles() {
    try {
      localStorage.setItem(recentStorageKey, JSON.stringify(recentFiles));
    } catch {
      showNotice("无法保存最近打开记录");
    }
  }

  function rememberFile(path: string, name: string) {
    const key = pathKey(path);
    recentFiles = [
      { path, name, openedAt: Date.now() },
      ...recentFiles.filter((file) => pathKey(file.path) !== key),
    ].slice(0, maxRecentFiles);
    saveRecentFiles();
  }

  function removeRecent(path: string) {
    const key = pathKey(path);
    recentFiles = recentFiles.filter((file) => pathKey(file.path) !== key);
    saveRecentFiles();
  }

  function clearRecentFiles() {
    recentFiles = [];
    saveRecentFiles();
    openMenu = null;
  }

  function restorePreferences() {
    const responsiveDefaultOutlineWidth = window.innerWidth <= 1100 ? 206 : defaultOutlineWidth;
    outlineWidth = responsiveDefaultOutlineWidth;
    try {
      const stored = localStorage.getItem(preferenceStorageKey);
      if (!stored) return;
      const preferences = JSON.parse(stored) as {
        darkMode?: boolean;
        tocVisible?: boolean;
        fontSize?: number;
        editorWordWrap?: boolean;
        editorIndentWidth?: number;
        outlineWidth?: number;
        editorSplitRatio?: number;
        editorPreviewVisible?: boolean;
      };
      darkMode = typeof preferences.darkMode === "boolean" ? preferences.darkMode : false;
      tocVisible = typeof preferences.tocVisible === "boolean" ? preferences.tocVisible : true;
      const savedOutlineWidth = preferences.outlineWidth;
      outlineWidth =
        typeof savedOutlineWidth === "number" && Number.isFinite(savedOutlineWidth)
          ? Math.min(maxOutlineWidth, Math.max(minOutlineWidth, savedOutlineWidth))
          : responsiveDefaultOutlineWidth;
      const savedFontSize = preferences.fontSize;
      fontSize =
        typeof savedFontSize === "number" && Number.isFinite(savedFontSize)
          ? Math.min(maxFontSize, Math.max(minFontSize, savedFontSize))
          : defaultFontSize;
      editorWordWrap =
        typeof preferences.editorWordWrap === "boolean" ? preferences.editorWordWrap : true;
      const savedIndentWidth = preferences.editorIndentWidth;
      editorIndentWidth =
        typeof savedIndentWidth === "number" &&
        editorIndentWidths.some((width) => width === savedIndentWidth)
          ? savedIndentWidth
          : defaultEditorIndentWidth;
      const savedSplitRatio = preferences.editorSplitRatio;
      editorSplitRatio =
        typeof savedSplitRatio === "number" && Number.isFinite(savedSplitRatio)
          ? Math.min(maxEditorSplitRatio, Math.max(minEditorSplitRatio, savedSplitRatio))
          : 0.5;
      editorPreviewVisible =
        typeof preferences.editorPreviewVisible === "boolean"
          ? preferences.editorPreviewVisible
          : false;
    } catch {
      try {
        localStorage.removeItem(preferenceStorageKey);
      } catch {
        // 存储不可用时继续使用默认设置
      }
    }
  }

  function savePreferences() {
    try {
      localStorage.setItem(
        preferenceStorageKey,
        JSON.stringify({
          darkMode,
          tocVisible,
          outlineWidth,
          fontSize,
          editorWordWrap,
          editorIndentWidth,
          editorSplitRatio,
          editorPreviewVisible,
        }),
      );
    } catch {
      showNotice("无法保存阅读设置");
    }
  }

  function lineEndingOf(source: string): string {
    let crlf = 0;
    let lf = 0;
    let cr = 0;
    for (let index = 0; index < source.length; index += 1) {
      const character = source.charCodeAt(index);
      if (character === 13) {
        if (source.charCodeAt(index + 1) === 10) {
          crlf += 1;
          index += 1;
        } else {
          cr += 1;
        }
      } else if (character === 10) {
        lf += 1;
      }
    }
    if (crlf >= lf && crlf >= cr && crlf > 0) return "\r\n";
    if (cr > lf && cr > 0) return "\r";
    return "\n";
  }

  function isDirty(tab: ReaderTab | null | undefined): boolean {
    if (!tab) return false;
    if (!tab.path) return sourceForTab(tab) !== tab.savedSource;
    if (tab.editorState && tab.savedEditorDoc) {
      return !tab.editorState.doc.eq(tab.savedEditorDoc);
    }
    return tab.source !== tab.savedSource;
  }

  function sourceForTab(tab: ReaderTab): string {
    const source = tab.editorState?.doc.toString();
    if (source === undefined || tab.lineEnding === "\n") return source ?? tab.source;
    return source.replace(/\n/g, tab.lineEnding);
  }

  function sameVersion(left: FileVersion | null, right: FileVersion | null): boolean {
    return !!left && !!right && left.size === right.size && left.modifiedNs === right.modifiedNs;
  }

  function isFileVersionConflict(message: string): boolean {
    return message.includes("文件已被删除或移动") || message.includes("其他程序");
  }

  function showWriteConflict(tab: ReaderTab, targetPath: string, message: string) {
    if (pathKey(targetPath) !== pathKey(tab.path)) {
      showNotice("另存为目标文件已变化，未覆盖。请重新选择位置后重试。");
    } else if (message.includes("删除或移动")) {
      tab.externalChange = "missing";
      showNotice("原文件已移动或删除，未覆盖。请另存为。");
    } else {
      tab.externalChange = "modified";
      showNotice("文件已被外部修改，未覆盖。请重新载入或另存为。");
    }
  }

  function persistSession() {
    try {
      const current = tabs.filter((tab) => !!tab.path).map((tab) => ({
        path: tab.path,
        scrollTop: Math.max(0, tab.scrollTop),
      }));
      localStorage.setItem(
        sessionStorageKey,
        JSON.stringify({
          tabs: current,
          activePath: tabs.find((tab) => tab.id === activeTabId && tab.path)?.path ?? null,
        } satisfies ReaderSession),
      );
    } catch {
      showNotice("无法保存阅读会话");
    }
  }

  function persistDraftRecovery(openTabs: ReaderTab[] = tabs) {
    const drafts = openTabs.flatMap((tab): DraftRecoveryRecord[] => {
      if (tab.path || tab.loading || tab.error) return [];
      const source = sourceForTab(tab);
      if (!source) return [];
      return [{
        name: tab.name,
        source,
        cursor: tab.cursor,
        editorScrollProgress: tab.editorScrollProgress,
        lineEnding: tab.lineEnding,
        updatedAt: tab.draftUpdatedAt || Date.now(),
      }];
    });

    try {
      if (drafts.length) {
        localStorage.setItem(draftRecoveryStorageKey, JSON.stringify(drafts));
      } else {
        localStorage.removeItem(draftRecoveryStorageKey);
      }
    } catch {
      showNotice("未命名文档暂存失败，建议另存为。");
    }
  }

  function scheduleDraftRecovery() {
    if (draftRecoveryTimer) clearTimeout(draftRecoveryTimer);
    draftRecoveryTimer = setTimeout(() => {
      draftRecoveryTimer = undefined;
      persistDraftRecovery();
    }, 800);
  }

  async function restoreDraftRecovery() {
    let records: DraftRecoveryRecord[] = [];
    try {
      const stored = localStorage.getItem(draftRecoveryStorageKey);
      if (!stored) return;
      const value: unknown = JSON.parse(stored);
      if (Array.isArray(value)) {
        records = value.filter(
          (record): record is DraftRecoveryRecord =>
            typeof record?.name === "string" &&
            typeof record?.source === "string" &&
            !!record.source &&
            typeof record?.cursor === "number" &&
            Number.isFinite(record.cursor) &&
            typeof record?.editorScrollProgress === "number" &&
            Number.isFinite(record.editorScrollProgress) &&
            typeof record?.lineEnding === "string" &&
            typeof record?.updatedAt === "number" &&
            Number.isFinite(record.updatedAt),
        );
      }
    } catch {
      try {
        localStorage.removeItem(draftRecoveryStorageKey);
      } catch {
        // 本地存储不可用时跳过草稿恢复
      }
      return;
    }

    if (!records.length) {
      try {
        localStorage.removeItem(draftRecoveryStorageKey);
      } catch {
        // 本地存储不可用时继续启动
      }
      return;
    }

    const restoredTabs = records
      .sort((left, right) => left.updatedAt - right.updatedAt)
      .map((record) => {
        const tab = createLoadingTab("", 0, true, false);
        tab.id = `recovered:${crypto.randomUUID()}`;
        tab.name = record.name || "未命名.md";
        tab.source = record.source;
        tab.savedSource = "";
        tab.editing = true;
        tab.cursor = Math.min(record.source.length, Math.max(0, Math.trunc(record.cursor)));
        tab.editorScrollProgress = Math.min(1, Math.max(0, record.editorScrollProgress));
        tab.lineEnding = ["\n", "\r\n", "\r"].includes(record.lineEnding)
          ? record.lineEnding
          : "\n";
        tab.draftUpdatedAt = record.updatedAt;
        return tab;
      });
    tabs = [...tabs, ...restoredTabs];
    if (!activeTabId) activeTabId = restoredTabs.at(-1)?.id ?? null;

    try {
      const markdown = await loadMarkdownModule();
      await Promise.all(restoredTabs.map(async (tab) => {
        const source = tab.source;
        const rendered = await markdown.renderMarkdown(
          source,
          loadDocumentImage(tab.path),
        );
        if (sourceForTab(tab) !== source) return;
        tab.html = rendered.html;
        tab.headings = rendered.headings;
      }));
      showNotice(`已恢复 ${restoredTabs.length} 份未保存文档`);
    } catch {
      restoredTabs.forEach((tab) => (tab.editing = true));
      showNotice("未保存文档已恢复，预览暂时无法生成。");
    }
  }

  function scheduleSessionSave() {
    if (restoringSession) return;
    if (sessionSaveTimer) clearTimeout(sessionSaveTimer);
    sessionSaveTimer = setTimeout(persistSession, 250);
  }

  async function restoreSession() {
    let session: ReaderSession | null = null;
    try {
      const stored = localStorage.getItem(sessionStorageKey);
      if (stored) {
        const parsed = JSON.parse(stored) as Partial<ReaderSession>;
        if (Array.isArray(parsed.tabs)) {
          const seen = new Set<string>();
          session = {
            tabs: parsed.tabs
              .filter(
                (tab): tab is SessionTab =>
                  typeof tab?.path === "string" &&
                  typeof tab?.scrollTop === "number" &&
                  Number.isFinite(tab.scrollTop),
              )
              .filter((tab) => {
                const key = pathKey(tab.path);
                if (seen.has(key)) return false;
                seen.add(key);
                return true;
              })
              .map((tab) => ({ path: tab.path, scrollTop: Math.max(0, tab.scrollTop) })),
            activePath: typeof parsed.activePath === "string" ? parsed.activePath : null,
          };
        }
      }
    } catch {
      try {
        localStorage.removeItem(sessionStorageKey);
      } catch {
        // 存储不可用时跳过会话恢复
      }
    }

    if (!session?.tabs.length) return;
    const activeSessionTab = session.activePath
      ? session.tabs.find((tab) => pathKey(tab.path) === pathKey(session.activePath!))
      : undefined;
    const firstTab = activeSessionTab ?? session.tabs.at(-1);
    if (!firstTab) return;

    restoringSession = true;
    try {
      for (const tab of session.tabs) {
        if (tabs.some((current) => pathKey(current.path) === pathKey(tab.path))) continue;
        tabs = [...tabs, createLoadingTab(tab.path, tab.scrollTop, false, false)];
      }

      await openPath(firstTab.path, undefined, firstTab.scrollTop);
    } finally {
      restoringSession = false;
      persistSession();
    }
  }

  onMount(() => {
    try {
      const stored = localStorage.getItem(recentStorageKey);
      if (stored) {
        const value = JSON.parse(stored);
        if (Array.isArray(value)) {
          recentFiles = value
            .filter(
              (item): item is RecentFile =>
                typeof item?.path === "string" && typeof item?.name === "string",
            )
            .slice(0, maxRecentFiles);
        }
      }
    } catch {
      try {
        localStorage.removeItem(recentStorageKey);
      } catch {
        // 存储不可用时跳过最近文件恢复
      }
    }

    restorePreferences();
    void restoreDraftRecovery();
    void restoreSession();
    const currentWindow = getCurrentWindow();
    void currentWindow.onDragDropEvent((event) => {
      if (event.payload.type !== "drop") return;
      const markdownPaths = event.payload.paths.filter((path) => /\.(?:md|markdown)$/i.test(path));
      if (markdownPaths.length < event.payload.paths.length) {
        showNotice("仅支持拖放 .md 和 .markdown 文件");
      }
      if (markdownPaths.length) void openFiles(markdownPaths);
    }).then((unlisten) => unlistenNativeEvents.push(unlisten));
    void currentWindow.onCloseRequested((event) => {
      if (allowConfirmedWindowClose) {
        allowConfirmedWindowClose = false;
        return;
      }
      persistSession();
      persistDraftRecovery();
      const dirtyIds = tabs.filter((tab) => isDirty(tab)).map((tab) => tab.id);
      if (!dirtyIds.length) return;
      event.preventDefault();
      showClosePrompt(dirtyIds, true);
    }).then((unlisten) => unlistenNativeEvents.push(unlisten));
    externalCheckTimer = setInterval(() => void checkOpenFiles(), 2500);

    const handleSaveAsKeydownCapture = (event: KeyboardEvent) => {
      if (event.defaultPrevented) return;
      const modifier = event.ctrlKey || event.metaKey;
      if (!modifier || !event.shiftKey || event.key.toLocaleLowerCase() !== "s") return;
      event.preventDefault();
      if (!pendingCloseIds.length) void saveActiveTab(true);
    };

    const handleKeydown = (event: KeyboardEvent) => {
      if (event.defaultPrevented) return;
      const modifier = event.ctrlKey || event.metaKey;
      const key = event.key.toLocaleLowerCase();

      if (event.key === "Escape") {
        if (zoomedImage) {
          zoomedImage = null;
        } else if (pendingCloseIds.length) {
          cancelPendingClose();
        } else if (tabContextMenu) {
          tabContextMenu = null;
        } else if (searchOpen) {
          closeSearch();
        } else {
          openMenu = null;
        }
        return;
      }

      if (modifier && key === "p") {
        event.preventDefault();
        if (!pendingCloseIds.length) void printActiveDocument();
        return;
      }

      if (pendingCloseIds.length) return;
      if (!modifier) return;
      if (key === "o") {
        event.preventDefault();
        void openFiles();
      } else if (key === "n") {
        event.preventDefault();
        void createMarkdownDocument();
      } else if (key === "w") {
        event.preventDefault();
        if (activeTabId) closeTab(activeTabId);
      } else if (key === "s") {
        event.preventDefault();
        if (event.shiftKey) void saveActiveTab(true);
        else void saveActiveTab();
      } else if (key === "f") {
        event.preventDefault();
        if (activeTab?.editing) editorComponent?.openSearch();
        else void showSearch();
      } else if (key === "=" || key === "+") {
        event.preventDefault();
        changeFontSize(1);
      } else if (key === "-") {
        event.preventDefault();
        changeFontSize(-1);
      } else if (key === "0") {
        event.preventDefault();
        fontSize = defaultFontSize;
        savePreferences();
      } else if (key === "tab") {
        event.preventDefault();
        cycleTab(event.shiftKey ? -1 : 1);
      }
    };

    window.addEventListener("keydown", handleSaveAsKeydownCapture, true);
    window.addEventListener("keydown", handleKeydown);
    return () => {
      window.removeEventListener("keydown", handleSaveAsKeydownCapture, true);
      window.removeEventListener("keydown", handleKeydown);
      for (const timer of editorPreviewTimers.values()) clearTimeout(timer);
      editorPreviewTimers.clear();
      if (noticeTimer) clearTimeout(noticeTimer);
      if (sessionSaveTimer) clearTimeout(sessionSaveTimer);
      if (draftRecoveryTimer) clearTimeout(draftRecoveryTimer);
      if (externalCheckTimer) clearInterval(externalCheckTimer);
      for (const unlisten of unlistenNativeEvents) unlisten();
    };
  });

  function toggleOutlinePanel() {
    tocVisible = !tocVisible;
    savePreferences();
  }

  async function createMarkdownDocument() {
    openMenu = null;
    closeSearch();

    let name = "未命名.md";
    let ordinal = 2;
    while (tabs.some((tab) => tab.name === name)) {
      name = `未命名 ${ordinal}.md`;
      ordinal += 1;
    }

    const tab = createLoadingTab("", 0, true, false);
    tab.id = `untitled:${crypto.randomUUID()}`;
    tab.name = name;
    tab.editing = true;
    tabs = [...tabs, tab];
    activeTabId = tab.id;
    activeHeadingId = null;
    scheduleSessionSave();

    await tick();
    if (activeTabId === tab.id) editorComponent?.focusEditor();
  }

  async function openFiles(paths?: string[]) {
    openMenu = null;
    try {
      const selection =
        paths ??
        (await open({
          multiple: true,
          filters: [{ name: "Markdown", extensions: ["md", "markdown"] }],
        }));
      if (!selection) return;
      const selectedPaths = Array.isArray(selection) ? selection : [selection];
      await Promise.all(selectedPaths.map((path) => openPath(path)));
    } catch {
      showNotice("文件选择失败，请重试。");
    }
  }

  async function openPath(
    path: string,
    fragment?: string,
    restoredScrollTop = 0,
    forceReload = false,
    activateOnOpen = true,
  ) {
    const initialId = pathKey(path);
    const existing = tabs.find((tab) => tab.id === initialId || pathKey(tab.path) === initialId);

    if (existing?.loading && existing.loadStarted) {
      if (fragment) existing.pendingFragment = fragment;
      if (activateOnOpen) await activateTab(existing.id);
      return;
    }

    if (existing && !forceReload && !existing.error) {
      const alreadyOpen = existing;
      if (!alreadyOpen.loadStarted) {
        if (fragment) alreadyOpen.pendingFragment = fragment;
        alreadyOpen.loadStarted = true;
        alreadyOpen.loading = true;
        if (activateOnOpen) activeTabId = alreadyOpen.id;
      } else {
        if (activateOnOpen) {
          await activateTab(alreadyOpen.id);
          if (fragment) await jumpToAnchor(fragment, alreadyOpen.id);
        }
        return;
      }
    }

    const loadTabId = existing?.id ?? initialId;
    const loadRequestId = crypto.randomUUID();
    const savedScrollTop = existing?.scrollTop ?? restoredScrollTop;
    const reloadWasEditing = forceReload && !!existing?.editing;
    const savedEditorScrollTop = existing?.editorScrollTop ?? 0;
    const savedEditorScrollProgress = existing?.editorScrollProgress ?? 0;
    const savedCursor = existing?.cursor ?? 0;
    if (existing) {
      existing.loadRequestId = loadRequestId;
      existing.loading = true;
      existing.loadStarted = true;
      existing.error = null;
      existing.externalChange = null;
      existing.editing = false;
      if (activateOnOpen) activeTabId = existing.id;
    } else {
      const loadingTab = createLoadingTab(path, restoredScrollTop, true);
      loadingTab.loadRequestId = loadRequestId;
      tabs = [...tabs, loadingTab];
      if (activateOnOpen) activeTabId = initialId;
      scheduleSessionSave();
    }
    openMenu = null;

    try {
      const [document, markdown] = await Promise.all([
        invoke<MarkdownDocument>("read_markdown", { path }),
        loadMarkdownModule(),
      ]);
      if (!tabs.some((tab) => tab.id === loadTabId && tab.loadRequestId === loadRequestId)) {
        return;
      }
      const canonicalId = pathKey(document.path);
      const duplicate = tabs.find(
        (tab) => tab.id === canonicalId && tab.loadRequestId !== loadRequestId,
      );
      if (duplicate) {
        const pendingFragment = fragment ?? duplicate.pendingFragment;
        const duplicateWasActive = activeTabId === loadTabId;
        if (duplicate.loading && pendingFragment) {
          duplicate.pendingFragment = pendingFragment;
        }
        tabs = tabs.filter(
          (tab) => !(tab.id === loadTabId && tab.loadRequestId === loadRequestId),
        );
        scheduleSessionSave();
        if (activateOnOpen || duplicateWasActive) {
          await activateTab(duplicate.id);
          if (pendingFragment && !duplicate.loading) {
            await jumpToAnchor(pendingFragment, duplicate.id);
          }
        }
        return;
      }

      const rendered = await markdown.renderMarkdown(document.content, (source) =>
        invoke<string>("read_local_image", {
          documentPath: document.path,
          source,
        }),
      );
      const currentRequest = tabs.find(
        (tab) => tab.id === loadTabId && tab.loadRequestId === loadRequestId,
      );
      if (!currentRequest) return;
      const duplicateAfterRender = tabs.find(
        (tab) => tab.id === canonicalId && tab.loadRequestId !== loadRequestId,
      );
      if (duplicateAfterRender) {
        const pendingFragment = fragment ?? duplicateAfterRender.pendingFragment;
        const duplicateWasActive = activeTabId === loadTabId;
        if (duplicateAfterRender.loading && pendingFragment) {
          duplicateAfterRender.pendingFragment = pendingFragment;
        }
        tabs = tabs.filter(
          (tab) => !(tab.id === loadTabId && tab.loadRequestId === loadRequestId),
        );
        scheduleSessionSave();
        if (activateOnOpen || duplicateWasActive) {
          await activateTab(duplicateAfterRender.id);
          if (pendingFragment && !duplicateAfterRender.loading) {
            await jumpToAnchor(pendingFragment, duplicateAfterRender.id);
          }
        }
        return;
      }
      const pendingFragment = fragment ?? currentRequest.pendingFragment;
      const shouldActivate = activeTabId === loadTabId;
      const editorInstanceId = crypto.randomUUID();

      tabs = tabs.map((tab) =>
        tab.id === loadTabId && tab.loadRequestId === loadRequestId
          ? {
              ...tab,
              id: canonicalId,
              editorInstanceId,
              editorState: null,
              editorCompartments: null,
              savedEditorDoc: null,
              previewedEditorDoc: null,
              previewedEditorPath: null,
              path: document.path,
              name: document.name,
              source: document.content,
              savedSource: document.content,
              html: rendered.html,
              headings: rendered.headings,
              scrollTop: savedScrollTop,
              editorScrollTop: forceReload ? savedEditorScrollTop : 0,
              editorScrollProgress: forceReload ? savedEditorScrollProgress : 0,
              cursor: forceReload ? savedCursor : 0,
              encoding: document.encoding,
              lineEnding: lineEndingOf(document.content),
              version: document.version,
              externalChange: null,
              editing: reloadWasEditing,
              loadRequestId: null,
              loading: false,
              error: null,
              pendingFragment: shouldActivate ? undefined : pendingFragment,
            }
          : tab,
      );
      if (shouldActivate) {
        activeTabId = canonicalId;
        activeHeadingId = rendered.headings[0]?.id ?? null;
      }
      rememberFile(document.path, document.name);
      await tick();
      const loadedTabIsCurrent = tabs.some(
        (tab) => tab.id === canonicalId && tab.editorInstanceId === editorInstanceId,
      );
      if (loadedTabIsCurrent && shouldActivate && activeTabId === canonicalId) {
        if (readerViewport) readerViewport.scrollTop = savedScrollTop;
        if (pendingFragment) await jumpToAnchor(pendingFragment, canonicalId);
      }
      scheduleSessionSave();
    } catch (error) {
      const message = displayError(error, "无法显示这篇文档，请重试。");
      const failedRequest = tabs.find(
        (tab) => tab.id === loadTabId && tab.loadRequestId === loadRequestId,
      );
      if (!failedRequest) return;
      if (forceReload && existing) {
        failedRequest.loading = false;
        failedRequest.loadRequestId = null;
        failedRequest.editing = reloadWasEditing;
        if (failedRequest.version) {
          const failedPath = failedRequest.path;
          const failedEditorInstanceId = failedRequest.editorInstanceId;
          await checkTabForExternalChange(failedRequest);
          const currentFailedRequest = tabs.find(
            (tab) =>
              tab.editorInstanceId === failedEditorInstanceId && tab.path === failedPath,
          );
          if (currentFailedRequest) {
            if (!currentFailedRequest.loadRequestId && !currentFailedRequest.externalChange) {
              currentFailedRequest.externalChange = "modified";
            }
            showNotice("重新载入失败，当前文档内容仍保留在窗口中。");
          }
        } else {
          failedRequest.error = message;
          failedRequest.externalChange = null;
          showNotice("无法打开文件，请检查文件是否存在且可读取。");
        }
      } else {
        tabs = tabs.map((tab) =>
          tab.id === loadTabId && tab.loadRequestId === loadRequestId
            ? { ...tab, loadRequestId: null, loading: false, error: message }
            : tab,
        );
        showNotice("无法打开文件，请检查文件是否存在且可读取。");
      }
      scheduleSessionSave();
    }
  }

  async function activateTab(id: string) {
    if (tabs.find((tab) => tab.id === id)?.editing && searchOpen) closeSearch();
    activeTabId = id;
    openMenu = null;
    tabContextMenu = null;
    scheduleSessionSave();
    const selectedBeforeLoad = tabs.find((tab) => tab.id === id);
    if (selectedBeforeLoad && !selectedBeforeLoad.loadStarted && !selectedBeforeLoad.error) {
      void openPath(selectedBeforeLoad.path, undefined, selectedBeforeLoad.scrollTop);
    }
    await tick();
    if (activeTabId !== id) return;
    const selected = tabs.find((tab) => tab.id === id);
    if (readerViewport && selected) readerViewport.scrollTop = selected.scrollTop;
    if (selected?.pendingFragment && !selected.loading && !selected.error) {
      const fragment = selected.pendingFragment;
      selected.pendingFragment = undefined;
      await jumpToAnchor(fragment, id);
    }
    syncActiveHeading();
    if (selected && !selected.loading && !selected.error) {
      void checkTabForExternalChange(selected);
    }
  }

  function showHomePage() {
    activeTabId = null;
    openMenu = null;
    tabContextMenu = null;
    activeHeadingId = null;
    closeSearch();
    scheduleSessionSave();
  }

  function closeTab(id: string) {
    requestCloseTabs([id]);
  }

  function createLoadingTab(
    path: string,
    scrollTop: number,
    loadStarted: boolean,
    loading = true,
  ): ReaderTab {
    return {
      id: pathKey(path),
      editorInstanceId: crypto.randomUUID(),
      editorState: null,
      editorCompartments: null,
      savedEditorDoc: null,
      previewedEditorDoc: null,
      previewedEditorPath: null,
      path,
      name: nameFromPath(path),
      source: "",
      savedSource: "",
      html: "",
      headings: [],
      scrollTop,
      editorScrollTop: 0,
      editorScrollProgress: 0,
      draftUpdatedAt: 0,
      cursor: 0,
      encoding: "utf-8",
      lineEnding: "\n",
      version: null,
      externalChange: null,
      editing: false,
      previewGeneration: 0,
      livePreviewGeneration: 0,
      loadRequestId: null,
      loading,
      loadStarted,
      error: null,
    };
  }

  function requestCloseTabs(ids: string[]) {
    const closing = ids.filter((id) => tabs.some((tab) => tab.id === id));
    if (!closing.length) return;
    if (closing.some((id) => isDirty(tabs.find((tab) => tab.id === id)))) {
      showClosePrompt(closing, false);
      return;
    }
    performCloseTabs(closing);
  }

  function showClosePrompt(ids: string[], closeWindowAfter: boolean) {
    closePromptGeneration += 1;
    pendingCloseIds = ids;
    closeAfterConfirmation = closeWindowAfter;
    void tick().then(() => {
      document.querySelector<HTMLElement>(".confirm-dialog")?.focus();
    });
  }

  function handleConfirmDialogKeydown(event: KeyboardEvent) {
    if (event.key !== "Tab") return;
    const dialog = event.currentTarget as HTMLElement;
    const buttons = Array.from(dialog.querySelectorAll<HTMLButtonElement>("button:not([disabled])"));
    if (!buttons.length) return;
    const first = buttons[0];
    const last = buttons.at(-1)!;
    if (document.activeElement === dialog) {
      event.preventDefault();
      (event.shiftKey ? last : first).focus();
      return;
    }
    if (event.shiftKey && document.activeElement === first) {
      event.preventDefault();
      last.focus();
    } else if (!event.shiftKey && document.activeElement === last) {
      event.preventDefault();
      first.focus();
    }
  }

  async function resolvePendingClose(action: "save" | "discard") {
    if (closeSaveInProgress) return;
    const promptGeneration = closePromptGeneration;
    let closing = [...pendingCloseIds];
    const shouldCloseWindow = closeAfterConfirmation;
    if (action === "discard") {
      closeSaveInProgress = true;
      try {
        while (true) {
          const pendingSaves = closing
            .map((id) => tabs.find((tab) => tab.id === id))
            .map((tab) => (tab ? saveOperations.get(tab) : undefined))
            .filter((operation): operation is Promise<boolean> => !!operation);
          if (!pendingSaves.length) break;
          await Promise.allSettled(pendingSaves);
          await Promise.resolve();
          if (closePromptGeneration !== promptGeneration) return;
          closing = [...pendingCloseIds];
        }
      } finally {
        closeSaveInProgress = false;
      }
    }
    if (action === "save") {
      closeSaveInProgress = true;
      try {
        for (const id of [...closing]) {
          if (closePromptGeneration !== promptGeneration) return;
          const tab = tabs.find((item) => item.id === id);
          if (tab && isDirty(tab)) {
            const saved = await saveTab(id);
            if (closePromptGeneration !== promptGeneration) return;
            if (!saved) return;
            if (tab.id !== id) {
              closing = closing.map((closingId) => (closingId === id ? tab.id : closingId));
              pendingCloseIds = pendingCloseIds.map((closingId) =>
                closingId === id ? tab.id : closingId,
              );
            }
          }
        }
        if (closePromptGeneration !== promptGeneration) return;
        if (closing.some((id) => isDirty(tabs.find((tab) => tab.id === id)))) {
          showNotice("保存期间文档又有修改，请再次保存后关闭。");
          return;
        }
      } finally {
        closeSaveInProgress = false;
      }
    }
    if (closePromptGeneration !== promptGeneration) return;
    pendingCloseIds = [];
    closeAfterConfirmation = false;
    closePromptGeneration += 1;
    if (shouldCloseWindow) {
      persistSession();
      persistDraftRecovery(
        action === "discard" ? tabs.filter((tab) => !closing.includes(tab.id)) : tabs,
      );
      allowConfirmedWindowClose = true;
      try {
        await getCurrentWindow().close();
      } catch (error) {
        allowConfirmedWindowClose = false;
        showNotice("关闭窗口失败：" + displayError(error, "请再试一次。"));
      }
      return;
    }
    performCloseTabs(closing);
  }

  function cancelPendingClose() {
    closePromptGeneration += 1;
    pendingCloseIds = [];
    closeAfterConfirmation = false;
  }

  function performCloseTabs(ids: string[]) {
    const closing = new Set(ids);
    const oldIndex = tabs.findIndex((tab) => tab.id === activeTabId);
    const wasActive = !!activeTabId && closing.has(activeTabId);
    for (const tab of tabs) {
      if (!closing.has(tab.id)) continue;
      const previewTimer = editorPreviewTimers.get(tab.editorInstanceId);
      if (previewTimer) clearTimeout(previewTimer);
      editorPreviewTimers.delete(tab.editorInstanceId);
    }
    tabs = tabs.filter((tab) => !closing.has(tab.id));
    persistDraftRecovery(tabs);

    if (wasActive) {
      const nextTab = tabs[Math.min(Math.max(0, oldIndex), tabs.length - 1)];
      activeTabId = nextTab?.id ?? null;
      activeHeadingId = nextTab?.headings[0]?.id ?? null;
      void tick().then(() => {
        if (activeTabId !== nextTab?.id) return;
        if (readerViewport && nextTab) readerViewport.scrollTop = nextTab.scrollTop;
        syncActiveHeading();
      });
    }
    if (tabs.length === 0) {
      activeTabId = null;
      searchOpen = false;
      searchQuery = "";
      activeHeadingId = null;
    }
    scheduleSessionSave();
  }

  function reorderTab(sourceId: string, targetId: string) {
    const sourceIndex = tabs.findIndex((tab) => tab.id === sourceId);
    const targetIndex = tabs.findIndex((tab) => tab.id === targetId);
    if (sourceIndex < 0 || targetIndex < 0 || sourceIndex === targetIndex) return;
    const reordered = [...tabs];
    const [moved] = reordered.splice(sourceIndex, 1);
    reordered.splice(targetIndex, 0, moved);
    tabs = reordered;
    scheduleSessionSave();
  }

  function handleTabDragStart(event: DragEvent, tabId: string) {
    draggedTabId = tabId;
    if (event.dataTransfer) {
      event.dataTransfer.effectAllowed = "move";
      event.dataTransfer.setData("text/plain", tabId);
    }
  }

  function handleTabDragOver(event: DragEvent) {
    if (!draggedTabId) return;
    event.preventDefault();
    if (event.dataTransfer) event.dataTransfer.dropEffect = "move";
  }

  function handleTabDrop(event: DragEvent, targetId: string) {
    if (!draggedTabId) return;
    event.preventDefault();
    reorderTab(draggedTabId, targetId);
    draggedTabId = null;
  }

  function showTabContextMenu(event: MouseEvent, tabId: string) {
    event.preventDefault();
    const target = event.currentTarget as HTMLElement;
    const rect = target.getBoundingClientRect();
    openTabContextMenu(tabId, event.clientX || rect.left, event.clientY || rect.bottom);
  }

  function openTabContextMenu(tabId: string, x: number, y: number) {
    openMenu = null;
    tabContextMenu = {
      tabId,
      x: Math.max(4, Math.min(x, window.innerWidth - 194)),
      y: Math.max(4, Math.min(y, window.innerHeight - 154)),
    };
    void tick().then(() => {
        document.querySelector<HTMLElement>(".tab-context-menu [role^='menuitem']")?.focus();
    });
  }

  function focusTabButton(tabId: string | null | undefined) {
    if (!tabId) return;
    void tick().then(() => {
      Array.from(document.querySelectorAll<HTMLButtonElement>(".tab-button"))
        .find((button) => button.dataset.tabId === tabId)
        ?.focus();
    });
  }

  async function copyTabPath(tabId: string) {
    const tab = tabs.find((item) => item.id === tabId);
    if (!tab?.path) return;
    try {
      await navigator.clipboard.writeText(displayPath(tab.path));
      showNotice("已复制文件路径");
    } catch {
      showNotice("无法复制文件路径");
    }
    tabContextMenu = null;
  }

  async function checkOpenFiles() {
    if (document.visibilityState === "hidden") return;
    const openTabs = tabs.filter((tab) => !tab.loading && !tab.error && tab.version);
    await Promise.all(openTabs.map((tab) => checkTabForExternalChange(tab)));
  }

  async function checkTabForExternalChange(tab: ReaderTab) {
    if (!tab.version || tab.loading) return;
    const checkedId = tab.id;
    if (externalChecksInFlight.has(checkedId)) return;
    externalChecksInFlight.add(checkedId);
    const checkedVersion = tab.version;
    try {
      const status = await invoke<MarkdownFileStatus>("get_markdown_version", { path: tab.path });
      const current = tabs.find((item) => item.id === checkedId);
      if (!current || !sameVersion(current.version, checkedVersion)) return;
      if (!status.version) {
        if (current.externalChange !== "missing") {
          current.externalChange = "missing";
          if (current.id === activeTabId) showNotice("文件已移动、删除或无法读取");
        }
      } else if (
        pathKey(current.path) !== pathKey(status.path) ||
        !sameVersion(current.version, status.version)
      ) {
        if (current.externalChange !== "modified") {
          current.externalChange = "modified";
          if (current.id === activeTabId) showNotice("文件已在其他程序中修改");
        }
      } else if (current.externalChange === "modified" || current.externalChange === "missing") {
        current.externalChange = null;
      }
    } catch {
      const current = tabs.find((item) => item.id === checkedId);
      if (
        current &&
        sameVersion(current.version, checkedVersion) &&
        current.externalChange !== "missing"
      ) {
        current.externalChange = "missing";
        if (current.id === activeTabId) showNotice("文件已移动、删除或无法读取");
      }
    } finally {
      externalChecksInFlight.delete(checkedId);
    }
  }

  async function reloadCurrentFile() {
    const tab = activeTab;
    if (!tab) return;
    const editorInstanceId = tab.editorInstanceId;
    if (isDirty(tab)) {
      const confirmed = await confirm("重新载入会放弃这篇文档的未保存修改。是否继续？", {
        title: "放弃未保存的修改？",
        kind: "warning",
      });
      if (!confirmed) return;
    }
    const confirmedSource = sourceForTab(tab);
    const confirmedEditorDoc = tab.editorState?.doc ?? null;

    while (true) {
      const currentTab = tabs.find((item) => item.editorInstanceId === editorInstanceId);
      if (!currentTab) return;
      const pendingSave = saveOperations.get(currentTab);
      if (!pendingSave) {
        await openPath(
          currentTab.path,
          undefined,
          currentTab.scrollTop,
          true,
          activeTabId === currentTab.id,
        );
        return;
      }
      try {
        await pendingSave;
      } catch {
        // 写入失败后仍按用户确认继续尝试重新载入
      }
      const latestTab = tabs.find((item) => item.editorInstanceId === editorInstanceId);
      if (!latestTab) return;
      const matchesConfirmedContent = confirmedEditorDoc && latestTab.editorState
        ? latestTab.editorState.doc.eq(confirmedEditorDoc)
        : sourceForTab(latestTab) === confirmedSource;
      if (!matchesConfirmedContent) {
        showNotice("等待写入期间文档又有修改，重新载入已取消。");
        return;
      }
    }
  }

  async function saveActiveTab(saveAs = false) {
    if (activeTabId) await saveTab(activeTabId, saveAs);
  }

  async function saveTab(
    tabId: string,
    saveAs = false,
    forcedEncoding?: string,
  ): Promise<boolean> {
    const tab = tabs.find((item) => item.id === tabId);
    if (!tab || tab.loading || tab.error) return false;
    if (!saveAs && tab.path && !isDirty(tab)) return true;

    const pendingSave = saveOperations.get(tab);
    if (pendingSave) {
      let saved: boolean;
      try {
        saved = await pendingSave;
      } catch {
        return false;
      }
      if (!saved) return false;
      const currentTab = tabs.find((item) => item.id === tab.id);
      if (!currentTab) return false;
      if (saveAs || isDirty(currentTab)) {
        return saveTab(currentTab.id, saveAs, forcedEncoding);
      }
      return true;
    }

    const operation = saveTabOperation(tabId, saveAs, forcedEncoding);
    saveOperations.set(tab, operation);
    try {
      return await operation;
    } catch (error) {
      showNotice("保存失败：" + displayError(error, "请检查文件权限和磁盘空间后重试。"));
      return false;
    } finally {
      if (saveOperations.get(tab) === operation) saveOperations.delete(tab);
    }
  }

  async function saveTabOperation(
    tabId: string,
    saveAs = false,
    forcedEncoding?: string,
  ): Promise<boolean> {
    const tab = tabs.find((item) => item.id === tabId);
    if (!tab || tab.loading || tab.error) return false;
    if (!saveAs && tab.path && !isDirty(tab)) return true;

    let targetPath = tab.path;
    let expectedVersion: FileVersion | null = tab.version;
    let encoding = forcedEncoding ?? tab.encoding;
    if (saveAs || !tab.path) {
      const selectedPath = await save({
        defaultPath: tab.path || tab.name,
        filters: [{ name: "Markdown", extensions: ["md", "markdown"] }],
      });
      if (!selectedPath) return false;
      let targetStatus: MarkdownFileStatus;
      try {
        targetStatus = await invoke<MarkdownFileStatus>("get_markdown_version", {
          path: selectedPath,
        });
      } catch (error) {
        showNotice("无法检查目标文件：" + displayError(error, "请检查路径和访问权限后重试。"));
        return false;
      }
      targetPath = targetStatus.path;
      const targetKey = pathKey(targetPath);
      if (tabs.some((other) => other.id !== tab.id && pathKey(other.path) === targetKey)) {
        showNotice("目标文件已在另一个标签中打开");
        return false;
      }

      const samePath = !!tab.path && targetKey === pathKey(tab.path);
      if (samePath) {
        expectedVersion = tab.version;
        encoding = forcedEncoding ?? tab.encoding;
      } else {
        encoding = forcedEncoding ?? tab.encoding;
        expectedVersion = targetStatus.version;
        if (expectedVersion) {
          const overwrite = await confirm("目标文件已存在。是否覆盖？", {
            title: "覆盖现有文件？",
            kind: "warning",
          });
          if (!overwrite) return false;
        }
      }
    }

    const editorStateToSave = tab.editorState;
    const sourceToSave = sourceForTab(tab);
    if (!saveAs && tab.path && tab.savedSource.length > 0 && sourceToSave.length === 0) {
      const confirmed = await confirm("当前内容为空，保存会清空原文件。确定继续吗？", {
        title: "清空文档？",
        kind: "warning",
      });
      if (!confirmed) return false;
    }
    const write = (writeEncoding: string) =>
      invoke<MarkdownWriteResult>("write_markdown", {
        path: targetPath,
        content: sourceToSave,
        encoding: writeEncoding,
        expectedVersion,
      });

    let result: MarkdownWriteResult;
    try {
      result = await write(encoding);
    } catch (error) {
      const message = displayError(error, "无法写入文件，请检查文件权限和磁盘空间后重试。");
      if (message.includes("无法表示新增字符")) {
        const convert = await confirm("当前编码无法保存新增字符。是否改用 UTF-8 保存？", {
          title: "转换为 UTF-8？",
          kind: "warning",
        });
        if (!convert) return false;
        try {
          result = await write("utf-8");
        } catch (retryError) {
          const retryMessage = displayError(
            retryError,
            "无法写入文件，请检查文件权限和磁盘空间后重试。",
          );
          if (isFileVersionConflict(retryMessage)) {
            showWriteConflict(tab, targetPath, retryMessage);
          } else {
            showNotice(retryMessage);
          }
          return false;
        }
      } else {
        if (isFileVersionConflict(message)) {
          showWriteConflict(tab, targetPath, message);
        } else {
          showNotice("保存失败：" + message);
        }
        return false;
      }
    }

    const previewPathChanged = pathKey(tab.path) !== pathKey(result.path);
    const previousId = tab.id;
    const nextId = pathKey(result.path);
    tab.id = nextId;
    if (activeTabId === previousId) activeTabId = nextId;
    if (pendingCloseIds.includes(previousId)) {
      pendingCloseIds = pendingCloseIds.map((id) => (id === previousId ? nextId : id));
    }
    tab.path = result.path;
    tab.name = result.name;
    tab.source = sourceToSave;
    tab.savedSource = sourceToSave;
    tab.savedEditorDoc = editorStateToSave?.doc ?? null;
    tab.encoding = result.encoding;
    tab.version = result.version;
    tab.externalChange = null;
    tab.error = null;
    rememberFile(result.path, result.name);
    persistDraftRecovery();
    scheduleSessionSave();
    if (previewPathChanged && tab.editing && tab.editorState) {
      scheduleEditorPreview(tab, tab.editorState);
    } else if (previewPathChanged) {
      const previewGeneration = ++tab.previewGeneration;
      void renderMarkdown(sourceToSave, loadDocumentImage(result.path))
        .then(async (rendered) => {
          if (
            !tabs.includes(tab) ||
            tab.editing ||
            tab.path !== result.path ||
            tab.previewGeneration !== previewGeneration
          ) return;
          tab.html = rendered.html;
          tab.headings = rendered.headings;
          if (activeTabId === tab.id) {
            activeHeadingId = rendered.headings[0]?.id ?? null;
            await tick();
            if (activeTabId === tab.id && !tab.editing) syncActiveHeading();
          }
        })
        .catch(() => {
          if (activeTabId === tab.id && !tab.editing) {
            showNotice("另存为后无法更新本地图片预览。");
          }
        });
    }
    showNotice("已保存");
    return true;
  }

  function handleEditorCursorChange(
    editorInstanceId: string,
    cursor: number,
    currentLine: number,
  ) {
    const tab = tabs.find((item) => item.editorInstanceId === editorInstanceId);
    if (!tab) return;
    tab.cursor = cursor;
    let low = 0;
    let high = tab.headings.length;
    while (low < high) {
      const middle = Math.floor((low + high) / 2);
      if (tab.headings[middle].line <= currentLine) low = middle + 1;
      else high = middle;
    }
    if (tab.id === activeTabId) {
      activeHeadingId = tab.headings[low - 1]?.id ?? null;
    }
  }

  function handleEditorScrollPositionChange(
    editorInstanceId: string,
    scrollTop: number,
    progress: number,
  ) {
    const tab = tabs.find((item) => item.editorInstanceId === editorInstanceId);
    if (!tab) return;
    tab.editorScrollTop = Math.max(0, scrollTop);
    tab.editorScrollProgress = Math.min(1, Math.max(0, progress));
    if (tab.id === activeTabId && tab.editing) setReaderScrollProgress(tab.editorScrollProgress);
  }

  function handleEditorStateChange(
    editorInstanceId: string,
    editorState: EditorState,
    compartments: EditorCompartments,
    documentChanged: boolean,
  ) {
    const tab = tabs.find((item) => item.editorInstanceId === editorInstanceId);
    if (!tab) return;
    if (!tab.savedEditorDoc) tab.savedEditorDoc = editorState.doc;
    const stateChanged = tab.editorState !== editorState;
    tab.editorState = editorState;
    tab.editorCompartments = compartments;
    if (documentChanged && stateChanged) {
      if (!tab.path) {
        tab.draftUpdatedAt = Date.now();
        scheduleDraftRecovery();
      }
      if (tab.editing) scheduleEditorPreview(tab, editorState);
    }
  }

  function scheduleEditorPreview(tab: ReaderTab, editorState: EditorState) {
    const previewKey = tab.editorInstanceId;
    const previousTimer = editorPreviewTimers.get(previewKey);
    if (previousTimer) clearTimeout(previousTimer);
    tab.previewedEditorDoc = null;
    tab.previewedEditorPath = null;
    const generation = ++tab.livePreviewGeneration;
    const documentPath = tab.path;
    const timer = setTimeout(() => {
      if (editorPreviewTimers.get(previewKey) === timer) editorPreviewTimers.delete(previewKey);
      if (
        !tabs.includes(tab) ||
        !tab.editing ||
        tab.editorState?.doc !== editorState.doc ||
        tab.path !== documentPath
      ) return;

      const source = sourceForTab(tab);
      void renderMarkdown(source, (imageSource) =>
        invoke<string>("read_local_image", {
          documentPath,
          source: imageSource,
        }),
      )
        .then(async (rendered) => {
          if (
            generation !== tab.livePreviewGeneration ||
            !tabs.includes(tab) ||
            !tab.editing ||
            tab.editorState?.doc !== editorState.doc ||
            tab.path !== documentPath
          ) return;
          tab.html = rendered.html;
          tab.headings = rendered.headings;
          tab.previewedEditorDoc = editorState.doc;
          tab.previewedEditorPath = documentPath;
          if (activeTabId === tab.id) {
            await tick();
            if (activeTabId !== tab.id || tab.editorState !== editorState) return;
            setReaderScrollProgress(tab.editorScrollProgress);
            syncActiveHeading();
          }
        })
        .catch(() => {
          if (generation === tab.livePreviewGeneration && activeTabId === tab.id) {
            showNotice("预览更新失败，编辑内容仍保留。");
          }
        });
    }, 420);
    editorPreviewTimers.set(previewKey, timer);
  }

  async function toggleEditing() {
    const tab = activeTab;
    if (!tab || tab.loading || tab.error) return;
    if (!tab.editing) {
      tab.previewGeneration += 1;
      if (!tab.editorState && readerViewport) {
        const maxScrollTop = readerViewport.scrollHeight - readerViewport.clientHeight;
        tab.editorScrollProgress =
          maxScrollTop > 0 ? readerViewport.scrollTop / maxScrollTop : 0;
      }
      closeSearch();
      tab.editing = true;
      await tick();
      if (activeTabId === tab.id && readerViewport) {
        readerViewport.scrollTop = tab.scrollTop;
        syncActiveHeading();
      }
      return;
    }

    const livePreviewTimer = editorPreviewTimers.get(tab.editorInstanceId);
    if (livePreviewTimer) clearTimeout(livePreviewTimer);
    editorPreviewTimers.delete(tab.editorInstanceId);
    if (activeTabId === tab.id && readerViewport) tab.scrollTop = readerViewport.scrollTop;
    tab.livePreviewGeneration += 1;
    const previewGeneration = ++tab.previewGeneration;

    const currentEditorState = tab.editorState;
    if (
      currentEditorState &&
      tab.previewedEditorDoc === currentEditorState.doc &&
      tab.previewedEditorPath === tab.path
    ) {
      tab.source = sourceForTab(tab);
      tab.editing = false;
      if (activeTabId === tab.id) {
        activeHeadingId = tab.headings[0]?.id ?? null;
        await tick();
        if (activeTabId === tab.id && readerViewport) {
          readerViewport.scrollTop = tab.scrollTop;
          syncActiveHeading();
        }
      }
      return;
    }

    try {
      let source: string;
      let rendered: RenderedDocument;
      while (true) {
        if (
          tab.previewGeneration !== previewGeneration ||
          !tabs.includes(tab) ||
          tab.loading ||
          !tab.editing
        ) return;

        const editorState = tab.editorState;
        const documentPath = tab.path;
        source = sourceForTab(tab);
        rendered = await renderMarkdown(source, (imageSource) =>
          invoke<string>("read_local_image", {
            documentPath,
            source: imageSource,
          }),
        );

        if (
          tab.previewGeneration !== previewGeneration ||
          !tabs.includes(tab) ||
          tab.loading ||
          !tab.editing
        ) return;
        if (tab.editorState === editorState && tab.path === documentPath) break;
      }

      tab.source = source;
      tab.html = rendered.html;
      tab.headings = rendered.headings;
      tab.previewedEditorDoc = tab.editorState?.doc ?? null;
      tab.previewedEditorPath = tab.path;
      tab.editing = false;
      if (activeTabId === tab.id) {
        activeHeadingId = rendered.headings[0]?.id ?? null;
        await tick();
        if (activeTabId === tab.id) {
          if (readerViewport) readerViewport.scrollTop = tab.scrollTop;
          syncActiveHeading();
        }
      }
    } catch (error) {
      if (tab.previewGeneration === previewGeneration) {
        showNotice("预览失败：" + displayError(error, "当前修改仍保留，请重试。"));
      }
    }
  }

  function cycleTab(direction: number) {
    if (tabs.length < 2 || !activeTabId) return;
    const currentIndex = tabs.findIndex((tab) => tab.id === activeTabId);
    const nextIndex = (currentIndex + direction + tabs.length) % tabs.length;
    void activateTab(tabs[nextIndex].id);
  }

  function handleReaderScroll(event: Event) {
    const viewport = event.currentTarget as HTMLDivElement;
    if (activeTab) {
      activeTab.scrollTop = viewport.scrollTop;
      if (activeTab.editing) {
        const maxScrollTop = viewport.scrollHeight - viewport.clientHeight;
        const progress = maxScrollTop > 0 ? viewport.scrollTop / maxScrollTop : 0;
        editorComponent?.scrollToProgress(progress);
      }
    }
    scheduleSessionSave();
    syncActiveHeading();
  }

  function setReaderScrollProgress(progress: number) {
    if (!readerViewport) return;
    const maxScrollTop = Math.max(0, readerViewport.scrollHeight - readerViewport.clientHeight);
    const nextScrollTop = maxScrollTop * Math.min(1, Math.max(0, progress));
    if (Math.abs(readerViewport.scrollTop - nextScrollTop) > 2) {
      readerViewport.scrollTop = nextScrollTop;
    }
  }

  function syncActiveHeading() {
    if (!readerViewport || !readerContent || !activeTab?.headings.length) {
      activeHeadingId = null;
      return;
    }

    const maxScrollTop = readerViewport.scrollHeight - readerViewport.clientHeight;
    if (maxScrollTop > 2 && readerViewport.scrollTop >= maxScrollTop - 2) {
      activeHeadingId = activeTab.headings.at(-1)?.id ?? null;
      return;
    }

    const viewportTop = readerViewport.getBoundingClientRect().top;
    const threshold = viewportTop + 92;
    let low = 0;
    let high = activeTab.headings.length;
    while (low < high) {
      const middle = Math.floor((low + high) / 2);
      const element = document.getElementById(activeTab.headings[middle].id);
      if (
        element &&
        readerContent.contains(element) &&
        element.getBoundingClientRect().top <= threshold
      ) {
        low = middle + 1;
      } else {
        high = middle;
      }
    }
    activeHeadingId = activeTab.headings[Math.max(0, low - 1)]?.id ?? null;
  }

  function jumpToHeading(heading: DocumentHeading) {
    if (activeTab?.editing) {
      editorComponent?.scrollToLine(heading.line);
      activeHeadingId = heading.id;
      return;
    }
    const element = document.getElementById(heading.id);
    if (element && readerContent?.contains(element)) {
      element.scrollIntoView({ behavior: scrollBehavior(), block: "start" });
      activeHeadingId = heading.id;
    }
  }

  async function jumpToAnchor(id: string, expectedTabId = activeTabId) {
    if (!expectedTabId || activeTabId !== expectedTabId) return;
    const expectedEditorInstanceId = tabs.find(
      (tab) => tab.id === expectedTabId,
    )?.editorInstanceId;
    if (!expectedEditorInstanceId) return;
    await tick();
    if (activeTabId !== expectedTabId) return;
    const expectedTab = tabs.find((tab) => tab.id === expectedTabId);
    if (!expectedTab || expectedTab.editorInstanceId !== expectedEditorInstanceId) return;
    const element = document.getElementById(id);
    if (!element || !readerContent?.contains(element)) return;
    element.scrollIntoView({ behavior: scrollBehavior(), block: "start" });
    if (expectedTab.headings.some((heading) => heading.id === id)) {
      activeHeadingId = id;
    }
  }

  function toggleTheme() {
    darkMode = !darkMode;
    savePreferences();
  }

  function changeFontSize(amount: number) {
    const nextFontSize = Math.min(maxFontSize, Math.max(minFontSize, fontSize + amount));
    if (nextFontSize === fontSize) return;
    fontSize = nextFontSize;
    savePreferences();
  }

  function bindFontZoomWheel(node: HTMLElement) {
    const handleWheel = (event: WheelEvent) => {
      if (!event.ctrlKey || event.deltaY === 0) return;
      event.preventDefault();

      const now = performance.now();
      if (now - lastFontZoomAt < 80) return;
      lastFontZoomAt = now;
      changeFontSize(event.deltaY < 0 ? 1 : -1);
    };

    node.addEventListener("wheel", handleWheel, { passive: false });
    return {
      destroy() {
        node.removeEventListener("wheel", handleWheel);
      },
    };
  }

  function toggleEditorWordWrap() {
    editorWordWrap = !editorWordWrap;
    savePreferences();
    openMenu = null;
    void tick().then(() => editorComponent?.focusEditor());
  }

  function setEditorIndentWidth(width: 2 | 4 | 8) {
    editorIndentWidth = width;
    savePreferences();
    openMenu = null;
    void tick().then(() => editorComponent?.focusEditor());
  }

  function beginEditorSplitResize(event: PointerEvent) {
    if (event.button !== 0) return;
    event.preventDefault();
    const splitter = event.currentTarget as HTMLElement;
    const bounds = editorSplitElement?.getBoundingClientRect();
    const splitterWidth = splitter.offsetWidth;
    const availableWidth = (bounds?.width ?? 0) - splitterWidth;
    if (!bounds || availableWidth <= 0) return;
    editorSplitResizeBounds = { left: bounds.left, splitterWidth, availableWidth };
    splitter.setPointerCapture(event.pointerId);
  }

  function resizeEditorSplit(event: PointerEvent) {
    const splitter = event.currentTarget as HTMLElement;
    const bounds = editorSplitResizeBounds;
    if (!splitter.hasPointerCapture(event.pointerId) || !bounds) return;
    editorSplitRatio = Math.min(
      maxEditorSplitRatio,
      Math.max(
        minEditorSplitRatio,
        (event.clientX - bounds.left - bounds.splitterWidth / 2) / bounds.availableWidth,
      ),
    );
  }

  function finishEditorSplitResize(event: PointerEvent) {
    const splitter = event.currentTarget as HTMLElement;
    if (splitter.hasPointerCapture(event.pointerId)) {
      resizeEditorSplit(event);
      splitter.releasePointerCapture(event.pointerId);
      editorSplitResizeBounds = null;
      savePreferences();
    } else {
      editorSplitResizeBounds = null;
    }
  }

  function handleEditorSplitKeydown(event: KeyboardEvent) {
    let nextRatio: number | null = null;
    if (event.key === "ArrowLeft") nextRatio = editorSplitRatio - 0.02;
    else if (event.key === "ArrowRight") nextRatio = editorSplitRatio + 0.02;
    else if (event.key === "Home") nextRatio = minEditorSplitRatio;
    else if (event.key === "End") nextRatio = maxEditorSplitRatio;
    else return;
    event.preventDefault();
    editorSplitRatio = Math.min(
      maxEditorSplitRatio,
      Math.max(minEditorSplitRatio, nextRatio),
    );
    savePreferences();
  }

  async function toggleEditorPreview() {
    editorPreviewVisible = !editorPreviewVisible;
    savePreferences();
    if (!editorPreviewVisible) return;

    const tabId = activeTabId;
    await tick();
    if (activeTabId === tabId && activeTab?.editing) {
      setReaderScrollProgress(activeTab.editorScrollProgress);
    }
  }

  function beginOutlineResize(event: PointerEvent) {
    if (event.button !== 0) return;
    event.preventDefault();
    const splitter = event.currentTarget as HTMLElement;
    const layout = splitter.parentElement;
    if (!layout) return;
    const bounds = layout.getBoundingClientRect();
    outlineResizeBounds = { left: bounds.left, splitterWidth: splitter.offsetWidth };
    splitter.setPointerCapture(event.pointerId);
  }

  function resizeOutline(event: PointerEvent) {
    const splitter = event.currentTarget as HTMLElement;
    const bounds = outlineResizeBounds;
    if (!splitter.hasPointerCapture(event.pointerId) || !bounds) return;
    outlineWidth = Math.min(
      maxOutlineWidth,
      Math.max(minOutlineWidth, event.clientX - bounds.left - bounds.splitterWidth / 2),
    );
  }

  function finishOutlineResize(event: PointerEvent) {
    const splitter = event.currentTarget as HTMLElement;
    if (splitter.hasPointerCapture(event.pointerId)) {
      resizeOutline(event);
      splitter.releasePointerCapture(event.pointerId);
      outlineResizeBounds = null;
      savePreferences();
    } else {
      outlineResizeBounds = null;
    }
  }

  function handleOutlineResizeKeydown(event: KeyboardEvent) {
    let nextWidth: number | null = null;
    if (event.key === "ArrowLeft") nextWidth = outlineWidth - 16;
    else if (event.key === "ArrowRight") nextWidth = outlineWidth + 16;
    else if (event.key === "Home") nextWidth = minOutlineWidth;
    else if (event.key === "End") nextWidth = maxOutlineWidth;
    else return;
    event.preventDefault();
    outlineWidth = Math.min(maxOutlineWidth, Math.max(minOutlineWidth, nextWidth));
    savePreferences();
  }

  async function showSearch() {
    if (!activeTab || activeTab.loading || activeTab.error || activeTab.editing) return;
    searchOpen = true;
    openMenu = null;
    await tick();
    searchInput?.focus();
    searchInput?.select();
  }

  function closeSearch() {
    searchOpen = false;
    searchQuery = "";
    searchCount = 0;
    activeSearchIndex = 0;
  }

  function clearSearchHighlights() {
    if (!readerContent) return;
    readerContent.querySelectorAll("mark.reader-search-hit").forEach((mark) => {
      mark.replaceWith(document.createTextNode(mark.textContent ?? ""));
    });
    readerContent.normalize();
    searchCount = 0;
    activeSearchIndex = 0;
  }

  function applySearchHighlights(query: string) {
    if (!readerContent) return;
    clearSearchHighlights();
    const term = query.trim();
    if (!term) return;
    const escapedTerm = term.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");

    const walker = document.createTreeWalker(readerContent, NodeFilter.SHOW_TEXT);
    const textNodes: Text[] = [];
    while (walker.nextNode()) {
      const node = walker.currentNode as Text;
      const parent = node.parentElement;
      if (!parent || parent.closest("script, style, mark")) continue;
      textNodes.push(node);
    }

    let matchIndex = 0;
    for (const node of textNodes) {
      const value = node.nodeValue ?? "";
      const matcher = new RegExp(escapedTerm, "giu");
      let match = matcher.exec(value);
      if (!match) continue;

      const fragment = document.createDocumentFragment();
      let cursor = 0;
      while (match) {
        const position = match.index;
        fragment.append(document.createTextNode(value.slice(cursor, position)));
        const mark = document.createElement("mark");
        mark.className = "reader-search-hit";
        mark.dataset.searchIndex = String(matchIndex);
        mark.textContent = match[0];
        fragment.append(mark);
        matchIndex += 1;
        cursor = position + match[0].length;
        match = matcher.exec(value);
      }
      fragment.append(document.createTextNode(value.slice(cursor)));
      node.parentNode?.replaceChild(fragment, node);
    }

    searchCount = matchIndex;
    activeSearchIndex = 0;
    setActiveSearchMatch(false);
  }

  function setActiveSearchMatch(scroll: boolean) {
    if (!readerContent || searchCount === 0) return;
    const marks = Array.from(
      readerContent.querySelectorAll<HTMLElement>("mark.reader-search-hit"),
    );
    marks.forEach((mark, index) => {
      mark.classList.toggle("current", index === activeSearchIndex);
    });
    if (scroll) {
      marks[activeSearchIndex]?.scrollIntoView({ behavior: "auto", block: "center" });
    }
  }

  function moveSearchMatch(direction: number) {
    if (searchCount === 0) return;
    activeSearchIndex =
      (activeSearchIndex + direction + searchCount) % searchCount;
    setActiveSearchMatch(true);
  }

  function handleSearchKeydown(event: KeyboardEvent) {
    if (event.key === "Enter") {
      event.preventDefault();
      moveSearchMatch(event.shiftKey ? -1 : 1);
    }
  }

  $effect(() => {
    const tabId = activeTab?.id;
    const isSearchOpen = searchOpen;
    const query = searchQuery;
    void tick().then(() => {
      if (
        activeTab?.id !== tabId ||
        searchOpen !== isSearchOpen ||
        searchQuery !== query
      ) return;
      if (!tabId || !isSearchOpen) clearSearchHighlights();
      else applySearchHighlights(query);
    });
  });

  $effect(() => {
    const selectedTabId = activeTabId;
    const strip = tabStripElement;
    if (!selectedTabId) return;
    void tick().then(() => {
      if (activeTabId !== selectedTabId) return;
      strip
        ?.querySelector<HTMLElement>(".tab-shell.selected")
        ?.scrollIntoView({ block: "nearest", inline: "nearest" });
    });
  });

  async function handleReaderClick(event: MouseEvent) {
    const target = event.target;
    if (!(target instanceof Element)) return;
    const copyButton = target.closest<HTMLButtonElement>("[data-copy-code]");
    if (copyButton) {
      event.preventDefault();
      const code = copyButton
        .closest(".reader-code-block")
        ?.querySelector("pre code")
        ?.textContent;
      if (code === undefined) return;
      try {
        await navigator.clipboard.writeText(code);
        const originalLabel = copyButton.dataset.copyLabel || copyButton.textContent?.trim() || "复制";
        const previousTimer = readerCodeCopyTimers.get(copyButton);
        if (previousTimer) clearTimeout(previousTimer);
        copyButton.dataset.copyLabel = originalLabel;
        copyButton.textContent = "已复制";
        const timer = window.setTimeout(() => {
          readerCodeCopyTimers.delete(copyButton);
          if (!copyButton.isConnected) return;
          copyButton.textContent = originalLabel;
          delete copyButton.dataset.copyLabel;
        }, 1400);
        readerCodeCopyTimers.set(copyButton, timer);
      } catch {
        showNotice("无法复制代码");
      }
      return;
    }
    const image = target.closest<HTMLImageElement>("img");
    if (image && !image.closest("a")) {
      event.preventDefault();
      zoomedImage = { src: image.currentSrc || image.src, alt: image.alt };
      return;
    }
    const anchor = target.closest("a");
    if (!anchor) return;

    const href = anchor.getAttribute("href");
    if (!href) return;
    event.preventDefault();

    if (href.startsWith("#")) {
      try {
        const id = decodeURIComponent(href.slice(1));
        if (id) await jumpToAnchor(id);
      } catch {
        showNotice("无法定位到这个标题。");
      }
      return;
    }

    if (/^https?:\/\//i.test(href) || /^mailto:/i.test(href)) {
      void openUrl(href).catch(() => showNotice("无法打开这个链接。"));
      return;
    }

    if (/^[a-z][a-z\d+.-]*:/i.test(href) || /^(?:\/|\\\\)/.test(href)) {
      showNotice("暂不支持这个链接格式。");
      return;
    }

    const hashIndex = href.indexOf("#");
    const pathAndQuery = hashIndex < 0 ? href : href.slice(0, hashIndex);
    const queryIndex = pathAndQuery.indexOf("?");
    const encodedPath = queryIndex < 0 ? pathAndQuery : pathAndQuery.slice(0, queryIndex);
    let path: string;
    let fragment: string | undefined;
    try {
      path = decodeURIComponent(encodedPath);
      fragment = hashIndex < 0 ? undefined : decodeURIComponent(href.slice(hashIndex + 1));
    } catch {
      showNotice("无法解析这个链接。");
      return;
    }

    if (!path) {
      if (fragment) await jumpToAnchor(fragment);
      return;
    }
    if (!/\.(?:md|markdown)$/i.test(path)) {
      showNotice("暂时只支持跳转到 Markdown 文档。");
      return;
    }

    const documentPath = activeTab?.path;
    if (!documentPath) return;
    try {
      const targetPath = await invoke<string>("resolve_markdown_link", {
        documentPath,
        source: path,
      });
      await openPath(targetPath, fragment);
    } catch {
      showNotice("无法打开链接中的 Markdown 文档。");
    }
  }

  function interceptReaderLinks(node: HTMLElement) {
    const handleClick = (event: MouseEvent) => void handleReaderClick(event);
    node.addEventListener("click", handleClick);
    return {
      destroy() {
        node.removeEventListener("click", handleClick);
      },
    };
  }

  function handleOuterPointerDown(event: PointerEvent) {
    const target = event.target;
    if (target instanceof Element && !target.closest(".menu-wrap")) {
      openMenu = null;
    }
    if (target instanceof Element && !target.closest(".tab-context-menu")) {
      tabContextMenu = null;
    }
  }

  function toggleMenu(menu: "file" | "recent" | "view" | "insert") {
    if (openMenu === menu) {
      openMenu = null;
      return;
    }

    openMenu = menu;
    void tick().then(() => {
      document.querySelector<HTMLElement>(".dropdown-menu [role^='menuitem']:not([disabled])")?.focus();
    });
  }

  function insertMarkdown(action: "bold" | "italic" | "inlineCode" | "codeBlock" | "link") {
    openMenu = null;
    editorComponent?.insertMarkdown(action);
  }

  function handleMenuKeydown(event: KeyboardEvent) {
    const menu = event.currentTarget as HTMLElement;
    const items = Array.from(
      menu.querySelectorAll<HTMLElement>(
        "[role='menuitem']:not([disabled]),[role='menuitemcheckbox']:not([disabled]),[role='menuitemradio']:not([disabled])",
      ),
    );
    if (!items.length) return;

    if (event.key === "Escape") {
      event.preventDefault();
      event.stopPropagation();
      if (menu.classList.contains("tab-context-menu")) {
        const tabId = tabContextMenu?.tabId;
        tabContextMenu = null;
        focusTabButton(tabId);
      } else {
        const trigger = menu.parentElement?.querySelector<HTMLButtonElement>(".menu-trigger");
        openMenu = null;
        void tick().then(() => trigger?.focus());
      }
      return;
    }

    const currentIndex = items.indexOf(document.activeElement as HTMLElement);
    let nextIndex: number | null = null;
    if (event.key === "ArrowDown") nextIndex = (currentIndex + 1 + items.length) % items.length;
    else if (event.key === "ArrowUp") {
      nextIndex = currentIndex <= 0 ? items.length - 1 : currentIndex - 1;
    } else if (event.key === "Home") nextIndex = 0;
    else if (event.key === "End") nextIndex = items.length - 1;
    else return;

    event.preventDefault();
    items[nextIndex]?.focus();
  }

  function handleTabKeydown(event: KeyboardEvent, tabId: string) {
    if (event.key === "ContextMenu" || (event.shiftKey && event.key === "F10")) {
      event.preventDefault();
      const rect = (event.currentTarget as HTMLElement).getBoundingClientRect();
      openTabContextMenu(tabId, rect.left, rect.bottom);
      return;
    }

    const currentIndex = tabs.findIndex((tab) => tab.id === tabId);
    if (currentIndex < 0) return;

    let nextIndex = currentIndex;
    if (event.key === "ArrowLeft") nextIndex = (currentIndex - 1 + tabs.length) % tabs.length;
    else if (event.key === "ArrowRight") nextIndex = (currentIndex + 1) % tabs.length;
    else if (event.key === "Home") nextIndex = 0;
    else if (event.key === "End") nextIndex = tabs.length - 1;
    else return;

    event.preventDefault();
    const nextId = tabs[nextIndex].id;
    void activateTab(nextId).then(() => {
      document.querySelectorAll<HTMLButtonElement>("[role='tab']")[nextIndex]?.focus();
    });
  }

  async function minimizeWindow() {
    await getCurrentWindow().minimize();
  }

  async function toggleMaximizeWindow() {
    await getCurrentWindow().toggleMaximize();
  }

  async function closeWindow() {
    try {
      await getCurrentWindow().close();
    } catch (error) {
      showNotice("关闭窗口失败：" + displayError(error, "请再试一次。"));
    }
  }
</script>

<svelte:head>
  <title>LeanReader</title>
</svelte:head>

<main class="app-shell" class:dark={darkMode} onpointerdown={handleOuterPointerDown}>
  <header class="titlebar">
    <div class="tab-strip" bind:this={tabStripElement}>
      <div class="tab-list" role="tablist" aria-label="已打开的文档">
        {#each tabs as tab (tab.id)}
          <div
            class="tab-shell"
            class:selected={tab.id === activeTabId}
            class:modified={isDirty(tab)}
          >
            <button
              class="tab-button"
              role="tab"
              data-tab-id={tab.id}
              draggable="true"
              aria-selected={tab.id === activeTabId}
              tabindex={tab.id === activeTabId ? 0 : -1}
              title={tab.path ? displayPath(tab.path) : tab.name}
              onclick={() => void activateTab(tab.id)}
              onkeydown={(event) => handleTabKeydown(event, tab.id)}
              oncontextmenu={(event) => showTabContextMenu(event, tab.id)}
              ondragstart={(event) => handleTabDragStart(event, tab.id)}
              ondragover={handleTabDragOver}
              ondrop={(event) => handleTabDrop(event, tab.id)}
              ondragend={() => (draggedTabId = null)}
            >
              <FileText size={16} strokeWidth={1.8} />
              <span class="tab-name">{tab.name}</span>
              {#if isDirty(tab)}<span class="dirty-indicator" aria-label="有未保存的修改"></span>{/if}
              {#if tab.externalChange}<span class="external-indicator" aria-label="文件在外部发生变化"></span>{/if}
              {#if tab.loading}<span class="tab-loading" aria-label="正在读取"></span>{/if}
            </button>
            <button
              class="tab-close"
              aria-label={"关闭 " + tab.name}
              title="关闭标签"
              onclick={() => closeTab(tab.id)}
            >
              <X size={14} strokeWidth={2} />
            </button>
          </div>
        {/each}
      </div>
    </div>
    <button
      class="add-tab-button"
      aria-label="打开或新建文件"
      title="打开或新建文件"
      onclick={showHomePage}
    >
      <Plus size={18} />
    </button>
    <div class="tab-drag-space drag-region" data-tauri-drag-region></div>

    <div class="window-controls">
      <button aria-label="最小化" title="最小化" onclick={() => void minimizeWindow()}>
        <Minus size={16} />
      </button>
      <button aria-label="最大化或还原" title="最大化或还原" onclick={() => void toggleMaximizeWindow()}>
        <Square size={12} />
      </button>
      <button class="close-window" aria-label="关闭窗口" title="关闭窗口" onclick={() => void closeWindow()}>
        <X size={16} />
      </button>
    </div>
  </header>

  {#if tabContextMenu}
    <div
      class="tab-context-menu"
      role="menu"
      tabindex="-1"
      style={`left:${tabContextMenu.x}px;top:${tabContextMenu.y}px`}
      transition:popupTransition={{ duration: 150, start: 0.97 }}
      onkeydown={handleMenuKeydown}
    >
      <button role="menuitem" onclick={() => { const id = tabContextMenu?.tabId; tabContextMenu = null; if (id) closeTab(id); }}>关闭标签</button>
      <button role="menuitem" onclick={() => { const id = tabContextMenu?.tabId; tabContextMenu = null; if (id) requestCloseTabs(tabs.filter((tab) => tab.id !== id).map((tab) => tab.id)); }}>关闭其他标签</button>
      {#if tabs.find((tab) => tab.id === tabContextMenu?.tabId)?.path}
        <div class="menu-divider"></div>
        <button role="menuitem" onclick={() => { const id = tabContextMenu?.tabId; if (id) void copyTabPath(id); }}>复制文件路径</button>
      {/if}
    </div>
  {/if}

  <nav class="toolbar" aria-label="阅读工具">
    <div class="toolbar-left">
      <div class="menu-wrap">
        <button class="menu-trigger" aria-haspopup="menu" aria-expanded={openMenu === "file"} onclick={() => toggleMenu("file")}>
          文件 <ChevronDown size={14} />
        </button>
        {#if openMenu === "file"}
          <div class="dropdown-menu" role="menu" tabindex="-1" transition:popupTransition={{ duration: 150, start: 0.97 }} onkeydown={handleMenuKeydown}>
            <button role="menuitem" onclick={() => void openFiles()}>
              <FolderOpen size={16} /> <span>打开文件…</span><kbd>Ctrl+O</kbd>
            </button>
            <button role="menuitem" onclick={() => void createMarkdownDocument()}>
              <Plus size={16} /> <span>新建文件</span><kbd>Ctrl+N</kbd>
            </button>
            {#if activeTab && !activeTab.loading && !activeTab.error}
              <button role="menuitem" onclick={() => { openMenu = null; void saveActiveTab(); }}>
                <Save size={16} /> <span>保存</span><kbd>Ctrl+S</kbd>
              </button>
              <button role="menuitem" onclick={() => { openMenu = null; void saveActiveTab(true); }}>
                <Save size={16} /> <span>另存为…</span><kbd>Ctrl+Shift+S</kbd>
              </button>
            {/if}
            {#if activeTab && !activeTab.loading && !activeTab.error}
              <div class="menu-divider"></div>
              <button role="menuitem" onclick={() => void printActiveDocument()}>
                <Printer size={16} /> <span>打印…</span><kbd>Ctrl+P</kbd>
              </button>
              <button role="menuitem" onclick={() => void exportActiveDocument()}>
                <Download size={16} /> <span>导出 HTML…</span>
              </button>
            {/if}
            <div class="menu-divider"></div>
            <button role="menuitem" onclick={() => void closeWindow()}>
              <LogOut size={16} /> <span>退出</span>
            </button>
          </div>
        {/if}
      </div>

      <div class="menu-wrap">
        <button class="menu-trigger" aria-haspopup="menu" aria-expanded={openMenu === "recent"} onclick={() => toggleMenu("recent")}>
          最近 <ChevronDown size={14} />
        </button>
        {#if openMenu === "recent"}
          <div class="dropdown-menu" role="menu" tabindex="-1" transition:popupTransition={{ duration: 150, start: 0.97 }} onkeydown={handleMenuKeydown}>
            {#if recentFiles.length}
              <span class="menu-label">最近打开</span>
              {#each recentFiles.slice(0, maxRecentFiles) as file (file.path)}
                <button role="menuitem" class="recent-menu-item" title={displayPath(file.path)} onclick={() => { openMenu = null; void openPath(file.path); }}>
                  <FileText size={16} /><span>{file.name}</span>
                </button>
              {/each}
              <div class="menu-divider"></div>
              <button role="menuitem" class="muted-menu-item" onclick={clearRecentFiles}>
                <Trash2 size={16} /><span>清空最近记录</span>
              </button>
            {:else}
              <span class="menu-label">暂无最近文件</span>
            {/if}
          </div>
        {/if}
      </div>

      <div class="menu-wrap">
        <button class="menu-trigger" aria-haspopup="menu" aria-expanded={openMenu === "view"} onclick={() => toggleMenu("view")}>
          查看 <ChevronDown size={14} />
        </button>
        {#if openMenu === "view"}
          <div class="dropdown-menu" role="menu" tabindex="-1" transition:popupTransition={{ duration: 150, start: 0.97 }} onkeydown={handleMenuKeydown}>
            <button role="menuitem" onclick={() => { toggleOutlinePanel(); openMenu = null; }}>
              {#if tocVisible}<PanelLeftClose size={16} />{:else}<PanelLeftOpen size={16} />{/if}
              <span>{tocVisible ? "隐藏目录" : "显示目录"}</span>
            </button>
            <div class="menu-divider"></div>
            <button role="menuitem" disabled={fontSize >= maxFontSize} onclick={() => { changeFontSize(1); openMenu = null; }}><Plus size={16} /><span>增加字号</span><kbd>Ctrl++</kbd></button>
            <button role="menuitem" disabled={fontSize <= minFontSize} onclick={() => { changeFontSize(-1); openMenu = null; }}><Minus size={16} /><span>减少字号</span><kbd>Ctrl+-</kbd></button>
            <button role="menuitem" title="按住 Ctrl 并滚动鼠标滚轮可调整字号（15–25）" onclick={() => { fontSize = defaultFontSize; savePreferences(); openMenu = null; }}><RotateCcw size={16} /><span>恢复字号</span><kbd>Ctrl+0</kbd></button>
            <div class="menu-divider"></div>
            <button role="menuitem" onclick={toggleTheme}>{#if darkMode}<Sun size={16} />{:else}<Moon size={16} />{/if}<span>{darkMode ? "浅色主题" : "深色主题"}</span></button>
            {#if activeTab?.editing}
              <div class="menu-divider"></div>
              <span class="menu-label">编辑器</span>
              <button role="menuitemcheckbox" aria-checked={editorWordWrap} onclick={toggleEditorWordWrap}>
                <TextWrap size={16} /><span>自动换行</span><kbd>{editorWordWrap ? "开" : "关"}</kbd>
              </button>
              <span class="menu-label">缩进宽度</span>
              {#each editorIndentWidths as width}
                <button role="menuitemradio" aria-checked={editorIndentWidth === width} onclick={() => setEditorIndentWidth(width)}>
                  {#if editorIndentWidth === width}<Check size={16} />{:else}<span class="menu-spacer"></span>{/if}
                  <span>{width} 个空格</span>
                </button>
              {/each}
            {/if}
          </div>
        {/if}
      </div>

      {#if activeTab?.editing}
        <div class="menu-wrap">
          <button class="menu-trigger" aria-haspopup="menu" aria-expanded={openMenu === "insert"} onclick={() => toggleMenu("insert")}>
            插入 <ChevronDown size={14} />
          </button>
          {#if openMenu === "insert"}
            <div class="dropdown-menu" role="menu" tabindex="-1" transition:popupTransition={{ duration: 150, start: 0.97 }} onkeydown={handleMenuKeydown}>
              <button role="menuitem" onclick={() => insertMarkdown("bold")}>
                <Bold size={16} /><span>粗体</span><kbd>Ctrl+B</kbd>
              </button>
              <button role="menuitem" onclick={() => insertMarkdown("italic")}>
                <Italic size={16} /><span>斜体</span><kbd>Ctrl+I</kbd>
              </button>
              <div class="menu-divider"></div>
              <button role="menuitem" onclick={() => insertMarkdown("inlineCode")}>
                <Code size={16} /><span>行内代码</span><kbd>Ctrl+Shift+K</kbd>
              </button>
              <button role="menuitem" onclick={() => insertMarkdown("codeBlock")}>
                <Code size={16} /><span>代码块</span><kbd>Ctrl+Shift+C</kbd>
              </button>
              <div class="menu-divider"></div>
              <button role="menuitem" onclick={() => insertMarkdown("link")}>
                <Link size={16} /><span>链接</span><kbd>Ctrl+K</kbd>
              </button>
            </div>
          {/if}
        </div>
        {/if}
    </div>

    <div class="toolbar-right">
      {#if activeTab?.editing}
        <button
          class="tool-button"
          aria-label="预览"
          title={editorPreviewVisible ? "隐藏预览" : "显示预览"}
          aria-pressed={editorPreviewVisible}
          onclick={toggleEditorPreview}
        >
          {#if editorPreviewVisible}<EyeOff size={18} />{:else}<Eye size={18} />{/if}
        </button>
      {/if}
      {#if activeTab && !activeTab.loading && !activeTab.error}
        <button
          class="tool-button mode-button"
          aria-label="阅读/编辑模式"
          title={activeTab.editing ? "阅读模式" : "编辑模式"}
          aria-pressed={activeTab.editing}
          onclick={() => void toggleEditing()}
        >
          {#if activeTab.editing}<BookOpen size={18} />{:else}<Pencil size={18} />{/if}
        </button>
        <span class="toolbar-separator"></span>
      {/if}
      <button
        class="tool-button"
        aria-label="文档目录"
        title={tocVisible ? "隐藏目录" : "显示目录"}
        aria-pressed={Boolean(activeTab && tocVisible)}
        onclick={toggleOutlinePanel}
      >
        {#if tocVisible}<PanelLeftClose size={18} />{:else}<PanelLeftOpen size={18} />{/if}
      </button>
      <button class="tool-button" aria-label="查找" title="查找当前文档 (Ctrl+F)" disabled={!activeTab || activeTab.loading || !!activeTab.error} onclick={() => { if (activeTab?.editing) editorComponent?.openSearch(); else void showSearch(); }}>
        <Search size={18} />
      </button>
      <span class="toolbar-separator"></span>
      <button class="tool-button theme-toggle" aria-label="深色主题" title={darkMode ? "浅色主题" : "深色主题"} aria-pressed={darkMode} onclick={toggleTheme}>
        {#if darkMode}<Sun size={18} />{:else}<Moon size={18} />{/if}
      </button>
    </div>

    {#if searchOpen && activeTab && !activeTab.loading && !activeTab.error && !activeTab.editing}
      <div
        class="search-box floating-search-box"
        role="search"
      >
        <Search size={16} />
        <input bind:this={searchInput} bind:value={searchQuery} placeholder="查找当前文档" aria-label="查找当前文档" onkeydown={handleSearchKeydown} />
        <span class="search-count" aria-live="polite" aria-atomic="true">{searchCount ? activeSearchIndex + 1 + " / " + searchCount : "0 条"}</span>
        <button class="search-step" aria-label="上一个匹配项" onclick={() => moveSearchMatch(-1)}><ChevronLeft size={16} /></button>
        <button class="search-step" aria-label="下一个匹配项" onclick={() => moveSearchMatch(1)}><ChevronRight size={16} /></button>
        <button class="search-close" aria-label="关闭查找" onclick={closeSearch}><X size={16} /></button>
      </div>
    {/if}
  </nav>

  <section
    class="reader-layout"
    class:with-outline={!!activeTab && tocVisible && !activeTab.loading && !activeTab.error}
    class:external-changed={!!activeTab?.externalChange && !activeTab.loading && !activeTab.error}
    style={"--outline-width:" + outlineWidth + "px"}
  >
    {#if activeTab?.externalChange && !activeTab.loading && !activeTab.error}
      <div class="external-banner" role="status" aria-live="polite">
        <span>
          {#if activeTab.externalChange === "missing"}
            原文件已移动、删除或无法读取。{#if isDirty(activeTab)}未保存修改仍保留在当前窗口。{/if}
          {:else}
            原文件已在其他程序中修改。{#if isDirty(activeTab)}保存前请重新载入或另存为。{/if}
          {/if}
        </span>
        <div>
          <button class="secondary-button" onclick={() => void reloadCurrentFile()}>重新载入</button>
          {#if isDirty(activeTab) || activeTab.externalChange === "missing"}
            <button class="secondary-button" onclick={() => void saveActiveTab(true)}>另存为…</button>
          {/if}
        </div>
      </div>
    {/if}

    {#if activeTab && tocVisible && !activeTab.loading && !activeTab.error}
      <aside class="outline-panel" aria-label="文档目录" in:panelTransition={{ x: -8, duration: 140 }}>
        {#if activeTab.headings.length}
          <nav class="outline-list" aria-label="标题目录">
            {#each activeTab.headings as heading (heading.id)}
              <button
                class="outline-item"
                class:current={activeHeadingId === heading.id}
                aria-current={activeHeadingId === heading.id ? "location" : undefined}
                style={"--heading-indent:" + ((heading.level - 1) * 14 + 10) + "px"}
                title={heading.title}
                onclick={() => jumpToHeading(heading)}
              >
                <span>{heading.title}</span>
              </button>
            {/each}
          </nav>
        {:else}
          <p class="outline-empty">这篇文档还没有标题</p>
        {/if}
      </aside>
      <div
        class="outline-splitter"
        in:fadeTransition={{ duration: 100 }}
        role="slider"
        aria-label="调整目录宽度"
        aria-orientation="vertical"
        aria-valuemin={minOutlineWidth}
        aria-valuemax={maxOutlineWidth}
        aria-valuenow={Math.round(outlineWidth)}
        aria-valuetext={`${Math.round(outlineWidth)} 像素`}
        tabindex="0"
        onpointerdown={beginOutlineResize}
        onpointermove={resizeOutline}
        onpointerup={finishOutlineResize}
        onpointercancel={finishOutlineResize}
        onkeydown={handleOutlineResizeKeydown}
      ></div>
    {/if}
    {#if !activeTab}
      <section class="home-screen">
        <div class="home-content">
          <div class="home-mark"><BookOpen size={28} strokeWidth={1.6} /></div>
          <div class="home-actions">
            <button class="primary-button" title="打开文件 (Ctrl+O)" onclick={() => void openFiles()}>
              <FolderOpen size={17} /> 打开文件
            </button>
            <button class="primary-button" title="新建文件 (Ctrl+N)" onclick={() => void createMarkdownDocument()}>
              <Plus size={17} /> 新建文件
            </button>
          </div>

          {#if recentFiles.length}
            <div class="recent-section">
              <div class="recent-heading">
                <span>最近打开</span>
                <button onclick={clearRecentFiles}>清空</button>
              </div>
              <ul class="recent-list">
                {#each recentFiles as file (file.path)}
                  <li>
                    <button class="recent-file" title={displayPath(file.path)} onclick={() => void openPath(file.path)}>
                      <span class="recent-file-icon"><FileText size={18} /></span>
                      <span class="recent-file-copy">
                        <strong>{file.name}</strong>
                        <small>{displayPath(file.path)}</small>
                      </span>
                    </button>
                    <button class="remove-recent" aria-label={"移除 " + file.name} title="从最近记录移除" onclick={() => removeRecent(file.path)}>
                      <X size={16} />
                    </button>
                  </li>
                {/each}
              </ul>
            </div>
          {/if}
        </div>
      </section>
    {:else if activeTab.loading}
      <section class="document-state">
        <div class="loading-ring" role="status" aria-label="正在加载文档"></div>
        <p>正在打开 {activeTab.name}</p>
      </section>
    {:else if activeTab.error}
      <section class="document-state error-state" role="alert">
        <div class="state-icon"><FileText size={22} /></div>
        <h2>无法打开这篇文档</h2>
        <p>{activeTab.error}</p>
        <div class="error-actions">
          <button class="secondary-button" onclick={() => void openPath(activeTab.path)}>重试</button>
          <button class="secondary-button" onclick={() => closeTab(activeTab.id)}>关闭标签</button>
        </div>
      </section>
    {:else if activeTab.editing}
      {@const editorInstanceId = activeTab.editorInstanceId}
      <div
        class="editor-split"
        in:fadeTransition={{ duration: 120 }}
        class:preview-visible={editorPreviewVisible}
        bind:this={editorSplitElement}
        style={editorPreviewVisible
          ? "grid-template-columns:minmax(0," + editorSplitRatio + "fr) 8px minmax(0," + (1 - editorSplitRatio) + "fr)"
          : "grid-template-columns:minmax(0,1fr)"}
      >
        <div class="editor-source-pane" role="region" aria-label="Markdown 源码编辑器">
          <div class="editor-viewport">
            {#key editorInstanceId}
              <MarkdownEditor
                bind:this={editorComponent}
                value={activeTab.source}
                editorState={activeTab.editorState}
                editorCompartments={activeTab.editorCompartments}
                cursor={activeTab.cursor}
                editorScrollProgress={activeTab.editorScrollProgress}
                lineEnding={activeTab.lineEnding}
                darkMode={darkMode}
                wordWrap={editorWordWrap}
                indentWidth={editorIndentWidth}
                onCursorChange={(cursor, line) =>
                  handleEditorCursorChange(editorInstanceId, cursor, line)
                }
                onScrollPositionChange={(scrollTop, progress) =>
                  handleEditorScrollPositionChange(editorInstanceId, scrollTop, progress)}
                onSave={(saveAs) => void saveActiveTab(saveAs)}
                onEditorStateChange={(state, compartments, documentChanged) =>
                  handleEditorStateChange(editorInstanceId, state, compartments, documentChanged)}
              />
            {/key}
          </div>
        </div>
        {#if editorPreviewVisible}
          <div
            class="editor-splitter"
            in:fadeTransition={{ duration: 100 }}
            role="slider"
            aria-label="调整源码与预览宽度"
            aria-orientation="vertical"
            aria-valuemin={Math.round(minEditorSplitRatio * 100)}
            aria-valuemax={Math.round(maxEditorSplitRatio * 100)}
            aria-valuenow={Math.round(editorSplitRatio * 100)}
            aria-valuetext={`${Math.round(editorSplitRatio * 100)}% 源码`}
            tabindex="0"
            onpointerdown={beginEditorSplitResize}
            onpointermove={resizeEditorSplit}
            onpointerup={finishEditorSplitResize}
            onpointercancel={finishEditorSplitResize}
            onkeydown={handleEditorSplitKeydown}
          ></div>
          <div
            class="editor-preview-pane"
            role="region"
            aria-label="Markdown 预览"
            in:panelTransition={{ x: 8, duration: 140 }}
          >
            <div class="reader-viewport" bind:this={readerViewport} onscroll={handleReaderScroll} use:bindFontZoomWheel>
              <article
                class="reader-content"
                bind:this={readerContent}
                style={"font-size:" + fontSize + "px"}
                use:interceptReaderLinks
              >
                {@html activeTab.html}
              </article>
            </div>
          </div>
        {/if}
      </div>
    {:else}
      <div class="reader-viewport" in:fadeTransition={{ duration: 120 }} bind:this={readerViewport} onscroll={handleReaderScroll} use:bindFontZoomWheel>
        <article
          class="reader-content"
          bind:this={readerContent}
          style={"font-size:" + fontSize + "px"}
          use:interceptReaderLinks
        >
          {@html activeTab.html}
        </article>
      </div>
    {/if}
  </section>

  {#if pendingCloseIds.length}
    <div class="modal-backdrop" transition:popupTransition={{ duration: 180, start: 1 }}>
      <div class="confirm-dialog" role="alertdialog" tabindex="-1" aria-modal="true" aria-labelledby="close-confirm-title" transition:popupTransition={{ duration: 180, start: 0.96 }} onkeydown={handleConfirmDialogKeydown}>
        <h2 id="close-confirm-title">有未保存的修改</h2>
        <p>
          {closeAfterConfirmation
            ? "退出前需要处理这些未保存文档。"
            : pendingCloseIds.length > 1
              ? "关闭前需要处理这些文档的未保存修改。"
              : "关闭前需要处理这篇文档的未保存修改。"}
        </p>
        {#if pendingCloseIds.length > 1}
          <ul class="pending-document-list">
            {#each pendingCloseIds as id (id)}
              {@const pendingTab = tabs.find((tab) => tab.id === id)}
              {#if pendingTab && isDirty(pendingTab)}<li>{pendingTab.name}</li>{/if}
            {/each}
          </ul>
        {/if}
        <div class="dialog-actions">
          <button class="secondary-button" onclick={cancelPendingClose}>取消</button>
          <button class="secondary-button" disabled={closeSaveInProgress} onclick={() => void resolvePendingClose("discard")}>放弃修改</button>
          <button class="primary-button" disabled={closeSaveInProgress} onclick={() => void resolvePendingClose("save")}>
            {closeSaveInProgress ? "正在保存…" : closeAfterConfirmation ? "保存并退出" : "保存并关闭"}
          </button>
        </div>
      </div>
    </div>
  {/if}

  {#if zoomedImage}
    <div class="image-zoom-backdrop" role="dialog" aria-modal="true" aria-label={zoomedImage.alt || "图片预览"} tabindex="-1" transition:popupTransition={{ duration: 180, start: 1 }}>
      <button class="image-zoom-close" aria-label="关闭图片预览" title="关闭图片预览" onclick={() => (zoomedImage = null)}>
        <X size={20} />
      </button>
      <img src={zoomedImage.src} alt={zoomedImage.alt} transition:popupTransition={{ duration: 180, start: 0.98 }} />
    </div>
  {/if}

  {#if notice}
    <div
      class="notice"
      role="status"
      aria-live="polite"
      transition:popupTransition={{ duration: 180, start: 0.97 }}
    >
      {notice}
    </div>
  {/if}
</main>

<style>
  :global(*) {
    box-sizing: border-box;
  }

  :global(html),
  :global(body) {
    width: 100%;
    height: 100%;
    margin: 0;
    overflow: hidden;
  }

  :global(body) {
    font-family: "Segoe UI Variable", "Segoe UI", "Microsoft YaHei UI", sans-serif;
    font-size: 15px;
    font-weight: 500;
    text-rendering: optimizeLegibility;
  }

  :global(button),
  :global(input) {
    font: inherit;
  }

  .app-shell {
    --ease-out: cubic-bezier(0.333333, 1, 0.666667, 1);
    --titlebar-height: 42px;
    --toolbar-height: 52px;
    --toolbar-inline-padding: 18px;
    --radius-control: 6px;
    --radius-popover: 8px;
    --radius-dialog: 12px;
    --duration-interaction: 140ms;
    --duration-press-in: 150ms;
    --duration-press-out: 110ms;
    --duration-surface: 180ms;
    --window-bg: #f4f6f8;
    --surface: #ffffff;
    --surface-muted: #f8f9fb;
    --text: #28313f;
    --muted: #596575;
    --subtle: #657182;
    --line: #e4e8ed;
    --line-strong: #d7dde5;
    --accent: #426ab8;
    --accent-foreground: #ffffff;
    --accent-hover: #3d65b2;
    --accent-soft: #edf3ff;
    --error: #a52229;
    --hover: #eef1f5;
    --code-bg: #f3f5f8;
    --code-comment: #596575;
    --code-keyword: #8255b5;
    --code-string: #2b704f;
    --code-number: #a14f27;
    --code-title: #286da5;
    --search-hit-bg: #fff1bc;
    --search-hit-text: #312a1c;
    --search-hit-current-bg: #f3c654;
    --search-hit-current-outline: rgba(111, 77, 18, 0.48);
    --shadow: 0 12px 35px rgba(29, 42, 61, 0.14);
    display: grid;
    grid-template-rows: var(--titlebar-height) var(--toolbar-height) minmax(0, 1fr);
    width: 100vw;
    height: 100vh;
    min-width: 0;
    color: var(--text);
    background: var(--window-bg);
    overflow: hidden;
  }

  .app-shell.dark {
    --window-bg: #191c21;
    --surface: #20242a;
    --surface-muted: #252a31;
    --text: #e7eaf0;
    --muted: #b8c1cc;
    --subtle: #a4adba;
    --line: #30363f;
    --line-strong: #3b434e;
    --accent: #8aa9ee;
    --accent-foreground: #15213b;
    --accent-hover: #b7c9f3;
    --accent-soft: #29364e;
    --error: #ff9095;
    --hover: #2a3038;
    --code-bg: #252a31;
    --code-comment: #b8c1cc;
    --code-keyword: #c5a4eb;
    --code-string: #76c895;
    --code-number: #f0a677;
    --code-title: #80b8e9;
    --search-hit-bg: #59471f;
    --search-hit-text: #fff2cc;
    --search-hit-current-bg: #765a1f;
    --search-hit-current-outline: rgba(255, 226, 153, 0.82);
    --shadow: 0 12px 35px rgba(0, 0, 0, 0.32);
  }

  .app-shell :global(button:focus-visible),
  .app-shell :global(a:focus-visible) {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }

  .app-shell :global(button) {
    --button-transform-duration: var(--duration-press-out);
    transition:
      color var(--duration-interaction) var(--ease-out),
      background-color var(--duration-interaction) var(--ease-out),
      border-color var(--duration-interaction) var(--ease-out),
      opacity var(--duration-interaction) var(--ease-out),
      box-shadow var(--duration-interaction) var(--ease-out),
      transform var(--button-transform-duration) var(--ease-out);
  }

  .app-shell :global(button:active:not(:disabled):not([draggable="true"])) {
    --button-transform-duration: var(--duration-press-in);
    transform: scale(0.97);
  }

  .app-shell,
  .titlebar,
  .toolbar,
  .tab-shell,
  .reader-layout,
  .outline-panel,
  .home-mark,
  .state-icon,
  .recent-list li,
  .recent-file-icon,
  .reader-viewport,
  .editor-split,
  .editor-preview-pane,
  .editor-viewport,
  .confirm-dialog,
  .search-box {
    transition:
      color var(--duration-surface) var(--ease-out),
      background-color var(--duration-surface) var(--ease-out),
      border-color var(--duration-surface) var(--ease-out);
  }

  .titlebar {
    --window-controls-width: 138px;
    display: flex;
    align-items: stretch;
    min-width: 0;
    background: var(--window-bg);
    border-bottom: 1px solid var(--line);
    user-select: none;
  }

  .drag-region {
    -webkit-app-region: drag;
  }

  .tab-strip {
    display: flex;
    flex: 0 1 auto;
    align-items: end;
    min-width: 0;
    max-width: calc(100% - var(--window-controls-width) - 44px);
    gap: 2px;
    padding: 5px 0 0 8px;
    overflow-x: auto;
    overflow-y: hidden;
    scrollbar-width: none;
  }

  .tab-strip::-webkit-scrollbar {
    display: none;
  }

  .tab-list {
    display: flex;
    flex: 0 0 auto;
    align-items: end;
    min-width: 0;
    height: 100%;
    gap: 2px;
  }

  .add-tab-button {
    display: inline-flex;
    flex: 0 0 30px;
    align-items: center;
    justify-content: center;
    width: 30px;
    height: 36px;
    margin-left: 2px;
    padding: 0;
    border: 0;
    border-radius: var(--radius-control);
    color: var(--muted);
    background: transparent;
    cursor: pointer;
  }

  .add-tab-button:hover {
    color: var(--text);
    background: var(--hover);
  }

  .add-tab-button:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: -2px;
  }

  .tab-shell {
    position: relative;
    display: flex;
    align-items: center;
    flex: 0 1 190px;
    min-width: 110px;
    max-width: 220px;
    height: 36px;
    border-radius: 8px 8px 0 0;
    color: var(--muted);
    transition: color var(--duration-interaction) var(--ease-out), background-color var(--duration-interaction) var(--ease-out);
  }

  .tab-shell.selected {
    color: var(--text);
    background: var(--surface);
    box-shadow: 0 1px 0 var(--surface);
  }

  .tab-shell:not(.selected):hover {
    background: var(--hover);
  }

  .tab-button,
  .tab-close,
  .window-controls button,
  .tool-button,
  .search-step,
  .search-close,
  .remove-recent {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    border: 0;
    color: inherit;
    background: transparent;
    cursor: pointer;
  }

  .tab-button {
    justify-content: flex-start;
    flex: 1;
    min-width: 0;
    height: 100%;
    gap: 8px;
    padding: 0 4px 0 11px;
    border-radius: inherit;
    color: inherit;
    text-align: left;
  }

  .tab-button :global(svg) {
    flex: 0 0 auto;
    color: var(--subtle);
  }

  .selected .tab-button :global(svg) {
    color: var(--accent);
  }

  .tab-name {
    overflow: hidden;
    font-size: 15px;
    font-weight: 550;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .dirty-indicator,
  .external-indicator {
    flex: 0 0 7px;
    width: 7px;
    height: 7px;
    border-radius: 50%;
  }

  .dirty-indicator {
    background: var(--accent);
  }

  .external-indicator {
    background: #d18b2c;
  }

  .tab-button[draggable="true"] {
    cursor: grab;
  }

  .tab-button[draggable="true"]:active {
    cursor: grabbing;
  }

  .tab-close {
    flex: 0 0 25px;
    width: 24px;
    height: 24px;
    margin-right: 5px;
    border-radius: var(--radius-control);
    opacity: 0.72;
  }

  .tab-close:hover,
  .remove-recent:hover {
    color: var(--text);
    background: var(--hover);
    opacity: 1;
  }

  .tab-loading {
    position: absolute;
    right: 30px;
    bottom: 0;
    left: 9px;
    height: 2px;
    overflow: hidden;
    pointer-events: none;
  }

  .tab-loading::after {
    position: absolute;
    inset: 0 auto 0 0;
    width: 38%;
    background: var(--accent);
    content: "";
    transform: translateX(0);
    animation: loading-progress 1.15s linear infinite;
  }

  @keyframes loading-progress {
    from { transform: translateX(-100%); }
    to { transform: translateX(265%); }
  }

  .window-controls button:hover,
  .tool-button:hover:not(:disabled),
  .search-step:hover,
  .search-close:hover {
    color: var(--text);
    background: var(--hover);
  }

  .tool-button[aria-pressed="true"]:not(:disabled) {
    color: var(--accent);
    background: var(--accent-soft);
  }

  .tab-drag-space {
    flex: 1;
    min-width: 12px;
    align-self: stretch;
  }

  .window-controls {
    display: flex;
    flex: 0 0 var(--window-controls-width);
    align-items: stretch;
    justify-content: flex-end;
  }

  .window-controls button {
    width: 46px;
    color: var(--muted);
  }

  .window-controls .close-window:hover {
    color: #fff;
    background: #d8343d;
  }

  .toolbar {
    position: relative;
    z-index: 5;
    display: flex;
    align-items: center;
    justify-content: space-between;
    min-width: 0;
    padding: 0 var(--toolbar-inline-padding);
    border-bottom: 1px solid var(--line);
    background: var(--surface);
  }

  .toolbar-left,
  .toolbar-right {
    display: flex;
    align-items: center;
    min-width: 0;
    gap: 5px;
  }

  .toolbar-left {
    gap: 8px;
  }

  .menu-wrap {
    position: relative;
  }

  .menu-trigger,
  .tool-button {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    height: 34px;
    padding: 0 9px;
    border: 0;
    border-radius: var(--radius-control);
    color: var(--muted);
    background: transparent;
    cursor: pointer;
  }

  .menu-trigger:hover {
    color: var(--text);
    background: var(--hover);
  }

  .menu-trigger[aria-expanded="true"] {
    color: var(--accent);
    background: var(--accent-soft);
  }

  .menu-trigger {
    font-size: 15px;
    font-weight: 550;
  }

  .toolbar-separator {
    width: 1px;
    height: 18px;
    margin: 0 5px;
    background: var(--line);
  }

  .toolbar-right {
    gap: 4px;
  }

  .tool-button {
    width: 36px;
    padding: 0;
  }

  .tool-button:disabled {
    color: var(--subtle);
    cursor: default;
    opacity: 0.5;
  }

  .dropdown-menu {
    position: absolute;
    top: 38px;
    left: 0;
    z-index: 10;
    display: flex;
    flex-direction: column;
    min-width: 236px;
    padding: 6px;
    border: 1px solid var(--line);
    border-radius: var(--radius-popover);
    background: var(--surface);
    box-shadow: var(--shadow);
    transform-origin: top left;
  }

  .tab-context-menu {
    position: fixed;
    z-index: 18;
    display: flex;
    flex-direction: column;
    width: 190px;
    padding: 5px;
    border: 1px solid var(--line);
    border-radius: var(--radius-popover);
    color: var(--text);
    background: var(--surface);
    box-shadow: var(--shadow);
    transform-origin: top left;
  }

  .tab-context-menu button {
    display: flex;
    align-items: center;
    min-height: 36px;
    padding: 0 10px;
    border: 0;
    border-radius: var(--radius-control);
    color: inherit;
    background: transparent;
    font-size: 15px;
    text-align: left;
    cursor: pointer;
  }

  .tab-context-menu button:hover {
    background: var(--hover);
  }

  .dropdown-menu button {
    display: flex;
    align-items: center;
    gap: 9px;
    min-height: 36px;
    padding: 0 9px;
    border: 0;
    border-radius: var(--radius-control);
    color: var(--text);
    background: transparent;
    text-align: left;
    cursor: pointer;
  }

  .dropdown-menu button:hover {
    background: var(--hover);
  }

  .dropdown-menu button:disabled {
    color: var(--subtle);
    opacity: 0.6;
    cursor: default;
  }

  .dropdown-menu button:disabled:hover {
    background: transparent;
  }

  .dropdown-menu button :global(svg) {
    flex: 0 0 16px;
    color: var(--muted);
  }

  .dropdown-menu button span:nth-child(2) {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .dropdown-menu kbd {
    color: var(--subtle);
    font-size: 15px;
    font-family: inherit;
  }

  .menu-divider {
    height: 1px;
    margin: 5px 3px;
    background: var(--line);
  }

  .menu-label {
    padding: 5px 9px 4px;
    color: var(--subtle);
    font-size: 15px;
    font-weight: 650;
  }

  .dropdown-menu .recent-menu-item {
    min-width: 0;
  }

  .dropdown-menu .muted-menu-item {
    color: var(--muted);
    font-size: 15px;
  }

  .menu-spacer {
    flex: 0 0 16px;
  }

  .search-box {
    display: flex;
    align-items: center;
    gap: 7px;
    width: min(420px, calc(100vw - 24px));
    height: 36px;
    padding: 0 5px 0 10px;
    box-sizing: border-box;
    border: 1px solid var(--line-strong);
    border-radius: var(--radius-popover);
    color: var(--muted);
    background: var(--surface);
  }

  .search-box:focus-within {
    border-color: var(--accent);
    box-shadow: 0 0 0 2px var(--accent-soft);
  }

  .floating-search-box {
    position: absolute;
    top: calc(100% + 6px);
    right: var(--toolbar-inline-padding);
    z-index: 20;
    box-shadow: var(--shadow);
    transform-origin: top right;
  }

  .search-box input {
    flex: 1;
    width: 100px;
    min-width: 48px;
    padding: 0;
    border: 0;
    outline: 0;
    color: var(--text);
    background: transparent;
  }

  .search-box input::placeholder {
    color: var(--subtle);
  }

  .search-count {
    flex: 0 0 auto;
    color: var(--subtle);
    font-size: 15px;
    white-space: nowrap;
  }

  .search-step,
  .search-close {
    flex: 0 0 28px;
    width: 28px;
    height: 28px;
    border-radius: var(--radius-control);
    color: var(--muted);
  }

  .reader-layout {
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    min-width: 0;
    min-height: 0;
    background: var(--window-bg);
  }

  .reader-layout.with-outline {
    grid-template-columns: minmax(176px, var(--outline-width)) 8px minmax(0, 1fr);
  }

  .reader-layout.external-changed {
    grid-template-rows: min-content minmax(0, 1fr);
  }

  .external-banner {
    grid-column: 1 / -1;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    min-height: 48px;
    padding: 6px 18px;
    border-bottom: 1px solid var(--line);
    color: var(--text);
    background: var(--accent-soft);
    font-size: 15px;
    line-height: 1.45;
  }

  .external-banner > div {
    display: flex;
    flex: 0 0 auto;
    align-items: center;
    gap: 8px;
  }

  .external-banner .secondary-button {
    min-height: 32px;
    margin: 0;
    padding: 0 11px;
    font-size: 15px;
  }

  .outline-panel {
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
    background: var(--window-bg);
  }

  .outline-splitter {
    position: relative;
    z-index: 1;
    width: 8px;
    min-height: 0;
    cursor: col-resize;
    touch-action: none;
    user-select: none;
  }

  .outline-splitter::after {
    position: absolute;
    top: 0;
    bottom: 0;
    left: 3px;
    width: 1px;
    background: var(--line);
    content: "";
  }

  .outline-splitter:hover::after,
  .outline-splitter:focus-visible::after {
    width: 2px;
    background: var(--accent);
  }

  .outline-splitter:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: -2px;
  }

  .outline-empty {
    margin: 0;
    padding: 10px 20px;
    color: var(--subtle);
    font-size: 14px;
    line-height: 1.6;
  }

  .outline-list {
    display: flex;
    flex-direction: column;
    gap: 2px;
    overflow: auto;
    padding: 3px 8px 24px 9px;
  }

  .outline-item {
    display: block;
    width: 100%;
    min-width: 0;
    min-height: 36px;
    padding: 6px 8px 6px var(--heading-indent);
    overflow: hidden;
    border: 0;
    border-radius: var(--radius-control);
    color: var(--muted);
    background: transparent;
    font-size: 15px;
    font-weight: 500;
    line-height: 1.45;
    text-align: left;
    text-overflow: ellipsis;
    white-space: nowrap;
    cursor: pointer;
  }

  .outline-item:hover {
    color: var(--text);
    background: var(--hover);
  }

  .outline-item.current {
    color: var(--accent);
    background: var(--accent-soft);
    font-weight: 600;
  }

  .home-screen,
  .document-state {
    display: flex;
    align-items: center;
    justify-content: center;
    min-width: 0;
    min-height: 0;
    overflow: auto;
  }

  .home-screen {
    padding: 50px 28px;
  }

  .home-content {
    display: flex;
    flex-direction: column;
    align-items: center;
    width: min(100%, 520px);
    margin: auto;
    text-align: center;
  }

  .home-mark,
  .state-icon {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 54px;
    height: 54px;
    border: 1px solid var(--line);
    border-radius: 16px;
    color: var(--accent);
    background: var(--surface);
  }

  .home-mark {
    margin-bottom: 28px;
  }

  .home-actions {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: center;
    gap: 10px;
    width: 100%;
  }

  .home-actions .primary-button {
    flex: 0 1 176px;
    min-height: 44px;
    margin: 0;
  }

  .primary-button,
  .secondary-button {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 9px;
    min-height: 44px;
    padding: 0 18px;
    border: 1px solid transparent;
    border-radius: var(--radius-control);
    color: var(--accent-foreground);
    background: var(--accent);
    font-size: 15px;
    font-weight: 600;
    cursor: pointer;
  }

  .primary-button:hover {
    background: var(--accent-hover);
  }

  .recent-section {
    width: 100%;
    margin-top: 38px;
    text-align: left;
  }

  .recent-heading {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 7px;
    padding: 0 5px;
    color: var(--muted);
    font-size: 15px;
    font-weight: 650;
  }

  .recent-heading button {
    padding: 4px 5px;
    border: 0;
    color: var(--subtle);
    background: transparent;
    font-size: 15px;
    cursor: pointer;
  }

  .recent-heading button:hover {
    color: var(--accent);
  }

  .recent-list {
    display: flex;
    flex-direction: column;
    gap: 3px;
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .recent-list li {
    display: flex;
    align-items: center;
    min-width: 0;
    min-height: 58px;
    padding: 3px 5px;
    border-radius: var(--radius-popover);
  }

  .recent-list li:hover {
    background: var(--surface);
  }

  .recent-file {
    display: flex;
    align-items: center;
    flex: 1;
    min-width: 0;
    gap: 11px;
    padding: 4px 6px;
    border: 0;
    color: var(--text);
    background: transparent;
    text-align: left;
    cursor: pointer;
  }

  .recent-file-icon {
    display: flex;
    align-items: center;
    justify-content: center;
    flex: 0 0 32px;
    width: 32px;
    height: 32px;
    border-radius: var(--radius-control);
    color: var(--accent);
    background: var(--accent-soft);
  }

  .recent-file-copy {
    display: flex;
    flex-direction: column;
    min-width: 0;
    gap: 4px;
  }

  .recent-file-copy strong,
  .recent-file-copy small {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .recent-file-copy strong {
    font-size: 15px;
    font-weight: 600;
  }

  .recent-file-copy small {
    color: var(--muted);
    font-size: 14px;
  }

  .remove-recent {
    flex: 0 0 26px;
    width: 26px;
    height: 26px;
    border-radius: var(--radius-control);
    color: var(--subtle);
    opacity: 0;
  }

  .recent-list li:hover .remove-recent,
  .remove-recent:focus-visible {
    opacity: 1;
  }

  .reader-viewport {
    min-width: 0;
    min-height: 0;
    overflow: auto;
    overscroll-behavior: contain;
    scrollbar-color: var(--line-strong) transparent;
    scrollbar-width: thin;
  }

  .editor-split {
    display: grid;
    width: 100%;
    height: 100%;
    min-width: 0;
    min-height: 0;
    grid-template-rows: minmax(0, 1fr);
    overflow: hidden;
    background: var(--surface);
  }

  .editor-source-pane,
  .editor-preview-pane {
    min-width: 0;
    min-height: 0;
    overflow: hidden;
  }

  .editor-source-pane {
    grid-row: 1;
  }

  .editor-split.preview-visible .editor-source-pane {
    border-right: 1px solid var(--line);
  }

  .editor-preview-pane {
    grid-row: 1;
    background: var(--surface);
  }

  .editor-preview-pane .reader-viewport {
    height: 100%;
  }

  .editor-splitter {
    grid-row: 1;
    position: relative;
    z-index: 1;
    width: 8px;
    min-height: 0;
    cursor: col-resize;
    background: var(--window-bg);
    touch-action: none;
    user-select: none;
  }

  .editor-splitter::after {
    position: absolute;
    top: 0;
    bottom: 0;
    left: 3px;
    width: 1px;
    background: var(--line);
    content: "";
  }

  .editor-splitter:hover::after,
  .editor-splitter:focus-visible::after {
    width: 2px;
    background: var(--accent);
  }

  .editor-splitter:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: -2px;
  }

  .editor-viewport {
    width: 100%;
    height: 100%;
    min-width: 0;
    min-height: 0;
    overflow: hidden;
    background: var(--surface);
  }

  .reader-content {
    width: min(100%, 780px);
    min-height: 100%;
    margin: 0 auto;
    padding: 52px 48px 100px;
    color: var(--text);
    font-size: 20px;
    font-weight: 450;
    line-height: 1.8;
    overflow-wrap: anywhere;
    user-select: text;
  }

  .reader-content :global(h1),
  .reader-content :global(h2),
  .reader-content :global(h3),
  .reader-content :global(h4),
  .reader-content :global(h5),
  .reader-content :global(h6) {
    color: var(--text);
    font-weight: 700;
    line-height: 1.35;
    letter-spacing: -0.025em;
    scroll-margin-top: 42px;
  }

  .reader-content :global(h1) {
    margin: 0 0 30px;
    font-size: 1.8em;
    letter-spacing: -0.04em;
  }

  .reader-content :global(h2) {
    margin: 2.15em 0 0.8em;
    padding-bottom: 0.35em;
    border-bottom: 1px solid var(--line);
    font-size: 1.33em;
  }

  .reader-content :global(h3) {
    margin: 1.8em 0 0.65em;
    font-size: 1.11em;
  }

  .reader-content :global(h4),
  .reader-content :global(h5),
  .reader-content :global(h6) {
    margin: 1.5em 0 0.6em;
    font-size: 1em;
  }

  .reader-content :global(p) {
    margin: 0 0 1.15em;
  }

  .reader-content :global(a) {
    color: var(--accent);
    text-decoration-color: color-mix(in srgb, var(--accent) 40%, transparent);
    text-underline-offset: 3px;
  }

  .reader-content :global(a:hover) {
    text-decoration-color: currentColor;
  }

  .reader-content :global(blockquote) {
    margin: 1.25em 0;
    padding: 0.12em 0 0.12em 1.05em;
    border-left: 3px solid var(--line-strong);
    color: var(--muted);
  }

  .reader-content :global(blockquote > :last-child) {
    margin-bottom: 0;
  }

  .reader-content :global(ul),
  .reader-content :global(ol) {
    margin: 0 0 1.2em;
    padding-left: 1.55em;
  }

  .reader-content :global(li) {
    padding-left: 0.2em;
  }

  .reader-content :global(li + li) {
    margin-top: 0.22em;
  }

  .reader-content :global(hr) {
    height: 1px;
    margin: 2.5em 0;
    border: 0;
    background: var(--line);
  }

  .reader-content :global(code) {
    padding: 0.16em 0.38em;
    border-radius: 4px;
    color: var(--text);
    background: var(--code-bg);
    font-family: "Cascadia Code", "Consolas", monospace;
    font-size: 0.92em;
  }

  .reader-content :global(pre) {
    margin: 1.35em 0 1.6em;
    padding: 17px 19px;
    overflow: auto;
    border: 1px solid var(--line);
    border-radius: 8px;
    background: var(--code-bg);
    line-height: 1.65;
  }

  .reader-content :global(pre code) {
    display: block;
    padding: 0;
    color: inherit;
    background: transparent;
    font-size: 0.9em;
    white-space: pre;
  }

  .reader-content :global(.reader-code-block) {
    position: relative;
    margin: 1.35em 0 1.6em;
  }

  .reader-content :global(.reader-code-block pre) {
    margin: 0;
    padding-top: 42px;
  }

  .reader-content :global(.reader-code-copy) {
    position: absolute;
    top: 8px;
    right: 9px;
    min-width: 54px;
    height: 27px;
    padding: 0 8px;
    border: 1px solid var(--line-strong);
    border-radius: var(--radius-control);
    color: var(--muted);
    background: var(--surface);
    font-size: 15px;
    cursor: pointer;
  }

  .reader-content :global(.reader-code-copy:hover) {
    color: var(--text);
    background: var(--hover);
  }

  .reader-content :global(.katex-display) {
    margin: 1.2em 0;
    overflow-x: auto;
    overflow-y: hidden;
    padding: 0.12em 0;
  }

  .reader-content :global(.katex-error) {
    color: var(--error);
    white-space: normal;
  }

  .reader-content :global(.math-block) {
    margin: 1.2em 0;
  }

  .reader-content :global(.mermaid-block) {
    min-width: 0;
    margin: 1.45em 0 1.7em;
    overflow-x: auto;
    text-align: center;
  }

  .reader-content :global(.mermaid-block > svg) {
    display: block;
    width: auto;
    max-width: 100%;
    max-height: 720px;
    height: auto;
    margin: 0 auto;
  }

  .reader-content :global(.mermaid-source) {
    margin-top: 0.65em;
    color: var(--muted);
    font-size: 14px;
    text-align: left;
  }

  .reader-content :global(.mermaid-source summary) {
    width: fit-content;
    cursor: pointer;
    font-weight: 550;
  }

  .reader-content :global(.mermaid-source pre) {
    margin-top: 0.6em;
  }

  .reader-content :global(.hljs-comment),
  .reader-content :global(.hljs-quote) {
    color: var(--code-comment);
    font-style: italic;
  }

  .reader-content :global(.hljs-keyword),
  .reader-content :global(.hljs-selector-tag),
  .reader-content :global(.hljs-literal),
  .reader-content :global(.hljs-name) {
    color: var(--code-keyword);
  }

  .reader-content :global(.hljs-string),
  .reader-content :global(.hljs-regexp),
  .reader-content :global(.hljs-addition) {
    color: var(--code-string);
  }

  .reader-content :global(.hljs-number),
  .reader-content :global(.hljs-attr),
  .reader-content :global(.hljs-variable),
  .reader-content :global(.hljs-template-variable) {
    color: var(--code-number);
  }

  .reader-content :global(.hljs-title),
  .reader-content :global(.hljs-section),
  .reader-content :global(.hljs-built_in) {
    color: var(--code-title);
  }

  .reader-content :global(table) {
    width: 100%;
    margin: 1.4em 0 1.8em;
    border-collapse: collapse;
    font-size: 0.95em;
  }

  .reader-content :global(th),
  .reader-content :global(td) {
    padding: 9px 12px;
    border: 1px solid var(--line-strong);
    text-align: left;
    vertical-align: top;
  }

  .reader-content :global(th) {
    background: var(--code-bg);
    font-weight: 650;
  }

  .reader-content :global(img) {
    display: block;
    max-width: 100%;
    height: auto;
    margin: 1.3em auto;
    border-radius: 5px;
    cursor: zoom-in;
  }

  .reader-content :global(a img) {
    cursor: pointer;
  }

  .reader-content :global(mark.reader-search-hit) {
    padding: 0 1px;
    border-radius: 2px;
    outline: 2px solid transparent;
    color: var(--search-hit-text);
    background: var(--search-hit-bg);
  }

  .reader-content :global(mark.reader-search-hit.current) {
    background: var(--search-hit-current-bg);
    outline: 2px solid var(--search-hit-current-outline);
  }

  .dark .reader-content :global(.hljs-comment),
  .dark .reader-content :global(.hljs-quote) {
    color: #9ba6b5;
  }

  .dark .reader-content :global(.hljs-keyword),
  .dark .reader-content :global(.hljs-selector-tag),
  .dark .reader-content :global(.hljs-literal),
  .dark .reader-content :global(.hljs-name) {
    color: #c9a4f2;
  }

  .dark .reader-content :global(.hljs-string),
  .dark .reader-content :global(.hljs-regexp),
  .dark .reader-content :global(.hljs-addition) {
    color: #8bd2ac;
  }

  .dark .reader-content :global(.hljs-number),
  .dark .reader-content :global(.hljs-attr),
  .dark .reader-content :global(.hljs-variable),
  .dark .reader-content :global(.hljs-template-variable) {
    color: #f0a978;
  }

  .dark .reader-content :global(.hljs-title),
  .dark .reader-content :global(.hljs-section),
  .dark .reader-content :global(.hljs-built_in) {
    color: #8fcaff;
  }

  .document-state {
    flex-direction: column;
    gap: 10px;
    padding: 28px;
    color: var(--muted);
    text-align: center;
  }

  .document-state p {
    margin: 0;
    font-size: 15px;
  }

  .loading-ring {
    width: 24px;
    height: 24px;
    border: 2px solid var(--line-strong);
    border-top-color: var(--accent);
    border-radius: 50%;
    animation: spin 800ms linear infinite;
  }

  @keyframes spin {
    to { transform: rotate(360deg); }
  }

  @media (prefers-reduced-motion: reduce) {
    .tab-loading::after,
    .loading-ring {
      animation: none !important;
    }

    .app-shell :global(button) {
      transition-property: color, background-color, border-color, opacity, box-shadow !important;
      transition-duration: 100ms !important;
      transition-timing-function: ease !important;
    }

    .app-shell :global(button:active:not(:disabled):not([draggable="true"])) {
      transform: none !important;
    }

    .app-shell,
    .titlebar,
    .toolbar,
    .tab-shell,
    .reader-layout,
    .outline-panel,
    .home-mark,
    .state-icon,
    .recent-list li,
    .recent-file-icon,
    .reader-viewport,
    .editor-split,
    .editor-preview-pane,
    .editor-viewport,
    .confirm-dialog,
    .search-box {
      transition-duration: 100ms !important;
      transition-timing-function: ease !important;
    }
  }

  .error-state h2 {
    margin: 2px 0 0;
    color: var(--text);
    font-size: 20px;
    font-weight: 600;
  }

  .error-state p {
    max-width: 520px;
    overflow-wrap: anywhere;
    line-height: 1.7;
  }

  .state-icon {
    width: 46px;
    height: 46px;
    border-radius: 13px;
  }

  .error-actions {
    display: flex;
    flex-wrap: wrap;
    justify-content: center;
    gap: 8px;
    margin-top: 8px;
  }

  .error-actions .secondary-button {
    margin-top: 0;
  }

  .secondary-button {
    min-height: 36px;
    margin-top: 8px;
    border-color: var(--line-strong);
    color: var(--text);
    background: var(--surface);
  }

  .secondary-button:hover {
    background: var(--hover);
  }

  .modal-backdrop {
    position: fixed;
    inset: 0;
    z-index: 40;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 24px;
    background: rgba(13, 20, 31, 0.34);
    transform-origin: center;
  }

  .image-zoom-backdrop {
    position: fixed;
    inset: 0;
    z-index: 50;
    display: grid;
    place-items: center;
    padding: 42px;
    background: rgba(8, 12, 19, 0.84);
    transform-origin: center;
  }

  .image-zoom-backdrop img {
    max-width: min(100%, 1440px);
    max-height: 100%;
    object-fit: contain;
    border-radius: 4px;
    box-shadow: 0 20px 70px rgba(0, 0, 0, 0.4);
    transform-origin: center;
  }

  .image-zoom-close {
    position: absolute;
    top: 14px;
    right: 16px;
    display: grid;
    width: 38px;
    height: 38px;
    place-items: center;
    border: 0;
    border-radius: var(--radius-control);
    color: #fff;
    background: rgba(255, 255, 255, 0.12);
    cursor: pointer;
  }

  .image-zoom-close:hover {
    background: rgba(255, 255, 255, 0.22);
  }

  .confirm-dialog {
    width: min(100%, 440px);
    padding: 24px;
    border: 1px solid var(--line);
    border-radius: var(--radius-dialog);
    color: var(--text);
    background: var(--surface);
    box-shadow: var(--shadow);
    transform-origin: center;
  }

  .confirm-dialog h2 {
    margin: 0;
    font-size: 19px;
    font-weight: 650;
  }

  .confirm-dialog p {
    margin: 10px 0 0;
    color: var(--muted);
    font-size: 15px;
    line-height: 1.6;
  }

  .pending-document-list {
    max-height: 150px;
    margin: 12px 0 0;
    padding-left: 21px;
    overflow: auto;
    color: var(--muted);
    font-size: 15px;
  }

  .dialog-actions {
    display: flex;
    flex-wrap: wrap;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 22px;
  }

  .dialog-actions button {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    min-height: 38px;
    margin: 0;
    padding: 0 13px;
    border: 1px solid var(--line-strong);
    border-radius: var(--radius-control);
    color: var(--text);
    background: var(--surface);
    font: inherit;
    font-size: 15px;
    font-weight: 600;
    cursor: pointer;
  }

  .dialog-actions button:hover:not(:disabled) {
    background: var(--hover);
  }

  .dialog-actions .primary-button {
    border-color: transparent;
    color: var(--accent-foreground);
    background: var(--accent);
  }

  .dialog-actions .primary-button:hover:not(:disabled) {
    background: var(--accent-hover);
  }

  .dialog-actions button:disabled {
    opacity: 0.6;
    cursor: default;
  }

  .notice {
    position: fixed;
    right: 24px;
    bottom: 22px;
    z-index: 20;
    max-width: min(420px, calc(100vw - 48px));
    padding: 11px 15px;
    border: 1px solid var(--line);
    border-radius: var(--radius-popover);
    color: var(--text);
    background: var(--surface);
    box-shadow: var(--shadow);
    font-size: 15px;
    line-height: 1.5;
    transform-origin: bottom right;
  }

  @media (max-width: 1100px) {
    .reader-content {
      padding-right: 36px;
      padding-left: 36px;
    }
  }

  @media (max-width: 820px) {
    .window-controls {
      flex-basis: 120px;
    }

    .window-controls button {
      width: 40px;
    }

    .app-shell {
      --toolbar-inline-padding: 9px;
    }

    .toolbar {
      padding: 0 var(--toolbar-inline-padding);
    }

    .reader-layout.with-outline {
      grid-template-columns: minmax(0, 1fr);
    }

    .outline-panel {
      display: none;
    }

    .outline-splitter {
      display: none;
    }

    .reader-content {
      padding: 40px 25px 76px;
    }

    .floating-search-box {
      right: var(--toolbar-inline-padding);
    }
  }

  @media (max-width: 560px) {
    .titlebar {
      --window-controls-width: 102px;
      height: 42px;
    }

    .window-controls button {
      width: 34px;
    }

    .tab-shell {
      min-width: 84px;
      max-width: 160px;
    }

    .toolbar-left {
      gap: 1px;
    }

    .toolbar-separator {
      margin: 0 2px;
    }

    .menu-trigger {
      padding: 0 6px;
    }

    .toolbar-right {
      gap: 0;
    }

    .tool-button {
      width: 30px;
    }

    .reader-content {
      padding: 32px 19px 64px;
    }

    .home-screen {
      padding: 34px 18px;
    }

    .recent-section {
      margin-top: 25px;
    }
  }

  @media print {
    @page {
      margin: 18mm;
    }

    :global(html),
    :global(body) {
      width: auto !important;
      height: auto !important;
      min-height: 0 !important;
      overflow: visible !important;
      background: #ffffff !important;
    }

    .app-shell,
    .app-shell.dark {
      --window-bg: #ffffff;
      --surface: #ffffff;
      --surface-muted: #f8f9fb;
      --text: #20242a;
      --muted: #424b57;
      --subtle: #424b57;
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
      display: block !important;
      width: auto !important;
      height: auto !important;
      min-width: 0 !important;
      overflow: visible !important;
      color: #20242a !important;
      background: #ffffff !important;
    }

    .titlebar,
    .toolbar,
    .outline-panel,
    .outline-splitter,
    .external-banner,
    .home-screen,
    .document-state,
    .modal-backdrop,
    .notice,
    .tab-context-menu {
      display: none !important;
    }

    .reader-content :global(.reader-code-copy),
    .image-zoom-backdrop {
      display: none !important;
    }

    .reader-layout {
      display: block !important;
      height: auto !important;
      min-height: 0 !important;
      overflow: visible !important;
      background: #ffffff !important;
    }

    .reader-viewport {
      height: auto !important;
      min-height: 0 !important;
      overflow: visible !important;
    }

    .reader-content {
      width: auto !important;
      max-width: none !important;
      min-height: 0 !important;
      margin: 0 !important;
      padding: 0 !important;
      color: #20242a !important;
    }

    .editor-split {
      display: block !important;
      height: auto !important;
      overflow: visible !important;
    }

    .editor-source-pane,
    .editor-splitter {
      display: none !important;
    }

    .editor-preview-pane {
      display: block !important;
      height: auto !important;
      overflow: visible !important;
      background: #ffffff !important;
    }

    .editor-preview-pane .reader-viewport {
      height: auto !important;
    }

    .reader-content :global(h1),
    .reader-content :global(h2),
    .reader-content :global(h3),
    .reader-content :global(h4) {
      break-after: avoid-page;
    }

    .reader-content :global(p),
    .reader-content :global(li) {
      orphans: 3;
      widows: 3;
    }

    .reader-content :global(pre) {
      overflow: visible !important;
      white-space: pre-wrap;
      overflow-wrap: anywhere;
      break-inside: avoid;
    }

    .reader-content :global(table),
    .reader-content :global(img),
    .reader-content :global(.mermaid-block) {
      break-inside: avoid;
    }

    .reader-content :global(mark.reader-search-hit) {
      color: inherit !important;
      background: transparent !important;
      outline: none !important;
    }

    .reader-content :global(.hljs-comment),
    .reader-content :global(.hljs-quote) {
      color: #4d5866 !important;
    }

    .reader-content :global(.hljs-keyword),
    .reader-content :global(.hljs-selector-tag),
    .reader-content :global(.hljs-literal),
    .reader-content :global(.hljs-name) {
      color: #63418e !important;
    }

    .reader-content :global(.hljs-string),
    .reader-content :global(.hljs-regexp),
    .reader-content :global(.hljs-addition) {
      color: #205c3f !important;
    }

    .reader-content :global(.hljs-number),
    .reader-content :global(.hljs-attr),
    .reader-content :global(.hljs-variable),
    .reader-content :global(.hljs-template-variable) {
      color: #87411f !important;
    }

    .reader-content :global(.hljs-title),
    .reader-content :global(.hljs-section),
    .reader-content :global(.hljs-built_in) {
      color: #205984 !important;
    }
  }
</style>

