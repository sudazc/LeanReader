<script lang="ts">
  import { onMount, tick } from "svelte";
  import { popupTransition } from "$lib/popup-transition";
  import type { Compartment, EditorState } from "@codemirror/state";
  import {
    Bold,
    ChevronDown,
    ChevronRight,
    ClipboardPaste,
    Code,
    Copy,
    Heading1,
    Italic,
    Link,
    Redo2,
    Scissors,
    TextSelect,
    Undo2,
  } from "lucide-svelte";

  type EditorCompartments = {
    lineWrapping: Compartment;
    indentation: Compartment;
  };
  type HeadingLevel = 1 | 2 | 3 | 4 | 5 | 6;
  type WrappedInsertAction = "bold" | "italic" | "inlineCode" | "link";
  type SearchButtonIcon = "previous" | "next" | "close";
  type InsertAction =
    | "bold"
    | "italic"
    | "inlineCode"
    | "codeBlock"
    | "link"
    | { type: "heading"; level: HeadingLevel };
  type FloatingPosition = { left: number; top: number; placement?: "left" | "right" };

  function createSearchIcon(name: SearchButtonIcon) {
    const namespace = "http://www.w3.org/2000/svg";
    const svg = document.createElementNS(namespace, "svg");
    svg.setAttribute("viewBox", "0 0 24 24");
    svg.setAttribute("width", "16");
    svg.setAttribute("height", "16");
    svg.setAttribute("fill", "none");
    svg.setAttribute("stroke", "currentColor");
    svg.setAttribute("stroke-width", "2");
    svg.setAttribute("stroke-linecap", "round");
    svg.setAttribute("stroke-linejoin", "round");
    svg.setAttribute("aria-hidden", "true");
    svg.setAttribute("focusable", "false");

    const paths: Record<SearchButtonIcon, string[]> = {
      previous: ["M15 18 9 12l6-6"],
      next: ["m9 18 6-6-6-6"],
      close: ["m18 6-12 12", "m6 6 12 12"],
    };
    for (const pathData of paths[name]) {
      const path = document.createElementNS(namespace, "path");
      path.setAttribute("d", pathData);
      svg.append(path);
    }
    return svg;
  }

  const headingLevels: HeadingLevel[] = [1, 2, 3, 4, 5, 6];

  type Props = {
    value: string;
    editorState: EditorState | null;
    editorCompartments: EditorCompartments | null;
    cursor: number;
    editorScrollProgress: number;
    lineEnding: string;
    onCursorChange: (cursor: number, line: number) => void;
    onScrollPositionChange: (scrollTop: number, progress: number) => void;
    onSave: (saveAs: boolean) => void;
    onEditorStateChange: (
      state: EditorState,
      compartments: EditorCompartments,
      documentChanged: boolean,
    ) => void;
    darkMode: boolean;
    wordWrap: boolean;
    indentWidth: number;
  };

  let {
    value,
    editorState,
    editorCompartments,
    cursor,
    editorScrollProgress,
    lineEnding,
    onCursorChange,
    onScrollPositionChange,
    onSave,
    onEditorStateChange,
    darkMode,
    wordWrap,
    indentWidth,
  }: Props = $props();
  let host = $state<HTMLDivElement>();
  let shell = $state<HTMLDivElement>();
  let editorLoading = $state(true);
  let loadingError = $state("");
  let selectionToolbarPosition = $state<FloatingPosition | null>(null);
  let selectionToolbarBelow = $state(false);
  let editorContextMenuPosition = $state<FloatingPosition | null>(null);
  let editorContextMenuError = $state("");
  let contextMenuHasSelection = $state(false);
  let contextMenuCanUndo = $state(false);
  let contextMenuCanRedo = $state(false);
  let selectionHeadingMenuOpen = $state(false);
  let selectionHeadingMenuAbove = $state(false);
  let contextHeadingMenuOpen = $state(false);
  let selectionToolbarElement = $state<HTMLDivElement>();
  let contextMenuElement = $state<HTMLDivElement>();
  let editorView: import("@codemirror/view").EditorView | undefined;
  let historyCommands:
    | {
        undo: (view: import("@codemirror/view").EditorView) => boolean;
        redo: (view: import("@codemirror/view").EditorView) => boolean;
        undoDepth: (state: EditorState) => number;
        redoDepth: (state: EditorState) => number;
      }
    | undefined;
  let mountedCompartments: EditorCompartments | undefined;
  let editorResizeObserver: ResizeObserver | undefined;
  let scrollFrame: number | undefined;
  let applyEditorPreferences:
    | ((nextWordWrap: boolean, nextIndentWidth: number) => void)
    | undefined;
  let searchPanelOpener:
    | ((view: import("@codemirror/view").EditorView) => boolean)
    | undefined;
  let selectionComesFromSearch = false;
  let searchRequested = false;
  let pendingInsertAction: InsertAction | null = null;

  $effect(() => {
    applyEditorPreferences?.(wordWrap, indentWidth);
  });

  function scrollProgress() {
    if (!editorView) return 0;
    const scrollDOM = editorView.scrollDOM;
    const maxScrollTop = scrollDOM.scrollHeight - scrollDOM.clientHeight;
    return maxScrollTop > 0
      ? Math.min(1, Math.max(0, scrollDOM.scrollTop / maxScrollTop))
      : 0;
  }

  function handleEditorScroll() {
    if (scrollFrame !== undefined) return;
    scrollFrame = requestAnimationFrame(() => {
      scrollFrame = undefined;
      if (editorView) {
        updateSelectionToolbar(editorView.state);
        onScrollPositionChange(editorView.scrollDOM.scrollTop, scrollProgress());
      }
    });
  }

  function updateSelectionToolbar(
    state: EditorState,
    isSearchSelection = selectionComesFromSearch,
  ) {
    if (isSearchSelection || !editorView || !shell || editorContextMenuPosition) {
      selectionToolbarPosition = null;
      selectionToolbarBelow = false;
      selectionHeadingMenuOpen = false;
      selectionHeadingMenuAbove = false;
      return;
    }

    const selection = state.selection.main;
    if (selection.empty) {
      selectionToolbarPosition = null;
      selectionToolbarBelow = false;
      selectionHeadingMenuOpen = false;
      selectionHeadingMenuAbove = false;
      return;
    }

    const caret =
      editorView.coordsAtPos(selection.to, -1) ??
      editorView.coordsAtPos(selection.from, 1);
    if (!caret) {
      selectionToolbarPosition = null;
      selectionToolbarBelow = false;
      selectionHeadingMenuOpen = false;
      selectionHeadingMenuAbove = false;
      return;
    }

    const bounds = shell.getBoundingClientRect();
    const toolbarWidth = 180;
    const toolbarHeight = 40;
    const centeredLeft = caret.left - bounds.left + (caret.right - caret.left) / 2 - toolbarWidth / 2;
    let left = centeredLeft;
    let top = caret.top - bounds.top - toolbarHeight - 8;
    let placement: FloatingPosition["placement"];
    selectionToolbarBelow = false;

    const lastLine = state.doc.lineAt(selection.to);
    const lineStart = editorView.coordsAtPos(lastLine.from, 1);
    const lineEnd = editorView.coordsAtPos(lastLine.to, -1);
    const previousLineEnd = lastLine.number > 1
      ? editorView.coordsAtPos(state.doc.line(lastLine.number - 1).to, -1)
      : null;
    const lineTop = lineEnd?.top ?? caret.top;
    const lineBottom = lineEnd?.bottom ?? caret.bottom;
    const lineHeight = Math.max(1, lineBottom - lineTop);
    const sideTop = lineTop - bounds.top - Math.max(0, toolbarHeight - lineHeight);
    const right = (lineEnd?.right ?? caret.right) - bounds.left + 8;
    const leftOfLine = lineStart
      ? lineStart.left - bounds.left - toolbarWidth - 8
      : -1;
    const previousLineEndX = previousLineEnd ? previousLineEnd.right - bounds.left : -1;
    const rightDoesNotCoverPreviousLine = !previousLineEnd || previousLineEndX <= right;
    const leftDoesNotCoverPreviousLine = !previousLineEnd || previousLineEndX <= leftOfLine;

    if (
      right + toolbarWidth <= bounds.width - 8 &&
      rightDoesNotCoverPreviousLine
    ) {
      left = right;
      top = sideTop;
      placement = "right";
    } else if (leftOfLine >= 8 && leftDoesNotCoverPreviousLine) {
      left = leftOfLine;
      top = sideTop;
      placement = "left";
    } else if (top < 8) {
      selectionToolbarBelow = true;
      top = caret.bottom - bounds.top + 8;
    }

    selectionToolbarPosition = {
      left: Math.min(Math.max(8, left), Math.max(8, bounds.width - toolbarWidth - 8)),
      top: Math.min(Math.max(8, top), Math.max(8, bounds.height - toolbarHeight - 8)),
      ...(placement ? { placement } : {}),
    };
  }

  function toggleSelectionHeadingMenu() {
    selectionHeadingMenuOpen = !selectionHeadingMenuOpen;
    if (!selectionHeadingMenuOpen || !shell || !selectionToolbarPosition) {
      selectionHeadingMenuAbove = false;
      return;
    }

    const belowSpace = shell.clientHeight - selectionToolbarPosition.top - 40;
    const aboveSpace = selectionToolbarPosition.top;
    selectionHeadingMenuAbove = belowSpace < 76 && aboveSpace > belowSpace;
  }

  async function handleEditorContextMenu(event: MouseEvent) {
    if (!editorView || !shell) return;
    event.preventDefault();

    const position = editorView.posAtCoords({ x: event.clientX, y: event.clientY });
    const currentSelection = editorView.state.selection.main;
    if (position !== null && (position < currentSelection.from || position > currentSelection.to)) {
      editorView.dispatch({ selection: { anchor: position } });
    }
    const selection = editorView.state.selection.main;

    const bounds = shell.getBoundingClientRect();
    const menuWidth = 230;
    const menuHeight = 430;
    selectionHeadingMenuOpen = false;
    selectionHeadingMenuAbove = false;
    contextHeadingMenuOpen = false;
    contextMenuHasSelection = !selection.empty;
    editorContextMenuError = "";
    contextMenuCanUndo = !!historyCommands?.undoDepth(editorView.state);
    contextMenuCanRedo = !!historyCommands?.redoDepth(editorView.state);
    editorContextMenuPosition = {
      left: Math.min(
        Math.max(8, event.clientX - bounds.left),
        Math.max(8, bounds.width - menuWidth - 8),
      ),
      top: Math.min(
        Math.max(8, event.clientY - bounds.top),
        Math.max(8, bounds.height - menuHeight - 8),
      ),
    };
    selectionToolbarPosition = null;
    editorView.focus();
    await tick();
    if (!editorContextMenuPosition || !contextMenuElement) return;
    const menuHeightAfterRender = contextMenuElement?.offsetHeight || menuHeight;
    editorContextMenuPosition = {
      left: editorContextMenuPosition?.left ?? 8,
      top: Math.min(
        Math.max(8, event.clientY - bounds.top),
        Math.max(8, bounds.height - menuHeightAfterRender - 8),
      ),
    };
    await tick();
    contextMenuElement?.focus();
  }

  function handleOutsidePointerDown(event: PointerEvent) {
    const target = event.target;
    if (!(target instanceof Node)) return;
    if (!contextMenuElement?.contains(target)) {
      editorContextMenuPosition = null;
      contextHeadingMenuOpen = false;
    }
    if (!editorView?.dom.contains(target) && !selectionToolbarElement?.contains(target)) {
      selectionToolbarPosition = null;
      selectionHeadingMenuOpen = false;
      selectionHeadingMenuAbove = false;
    }
  }

  function handleEditorPointerDown() {
    selectionToolbarPosition = null;
    selectionHeadingMenuOpen = false;
    selectionHeadingMenuAbove = false;
  }

  function handleEditorPointerUp() {
    requestAnimationFrame(() => {
      if (editorView) updateSelectionToolbar(editorView.state);
    });
  }

  function handleEditorKeyUp() {
    requestAnimationFrame(() => {
      if (editorView) updateSelectionToolbar(editorView.state);
    });
  }

  function handleEditorFocusOut(event: FocusEvent) {
    if (
      event.relatedTarget instanceof Node &&
      selectionToolbarElement?.contains(event.relatedTarget)
    ) return;
    const editorElement = event.currentTarget as HTMLElement;
    void tick().then(() => {
      if (
        !editorElement.isConnected ||
        editorView?.hasFocus ||
        (document.activeElement instanceof Node &&
          selectionToolbarElement?.contains(document.activeElement))
      ) return;
      selectionToolbarPosition = null;
      selectionHeadingMenuOpen = false;
      selectionHeadingMenuAbove = false;
    });
  }

  function handleContextMenuKeydown(event: KeyboardEvent) {
    if (event.key === "Escape") {
      event.preventDefault();
      event.stopPropagation();
      if (contextHeadingMenuOpen) {
        contextHeadingMenuOpen = false;
        contextMenuElement?.focus();
        return;
      }
      editorContextMenuPosition = null;
      editorView?.focus();
      return;
    }

    const items = Array.from(
      contextMenuElement?.querySelectorAll<HTMLButtonElement>("[role='menuitem']:not(:disabled)") ?? [],
    );
    const currentIndex = items.indexOf(document.activeElement as HTMLButtonElement);
    let nextIndex: number | null = null;
    if (event.key === "ArrowDown") nextIndex = (currentIndex + 1) % items.length;
    else if (event.key === "ArrowUp") {
      nextIndex = currentIndex < 0 ? items.length - 1 : (currentIndex - 1 + items.length) % items.length;
    }
    else if (event.key === "Home") nextIndex = 0;
    else if (event.key === "End") nextIndex = items.length - 1;
    if (nextIndex === null || !items.length) return;
    event.preventDefault();
    items[nextIndex]?.focus();
  }

  function insertFromContextMenu(action: InsertAction) {
    editorContextMenuPosition = null;
    contextHeadingMenuOpen = false;
    applyMarkdownInsertion(action);
  }

  function runHistoryCommand(action: "undo" | "redo") {
    if (!editorView || !historyCommands) return;
    historyCommands[action](editorView);
    editorContextMenuPosition = null;
    editorView.focus();
  }

  function selectAllFromContextMenu() {
    if (!editorView) return;
    editorView.dispatch({ selection: { anchor: 0, head: editorView.state.doc.length } });
    editorContextMenuPosition = null;
    editorView.focus();
    updateSelectionToolbar(editorView.state);
  }

  async function runClipboardAction(action: "cut" | "copy" | "paste") {
    if (!editorView) return;
    const view = editorView;
    try {
      if (action === "paste") {
        const clipboardText = await navigator.clipboard.readText();
        if (clipboardText) {
          const { from, to } = view.state.selection.main;
          const text = clipboardText.replace(/\r\n?/g, "\n");
          view.dispatch({
            changes: { from, to, insert: text },
            selection: { anchor: from + text.length },
            scrollIntoView: true,
          });
        }
      } else {
        const { from, to } = view.state.selection.main;
        if (from === to) return;
        const selectedText = view.state.sliceDoc(from, to);
        await navigator.clipboard.writeText(selectedText);
        if (action === "cut") {
          view.dispatch({
            changes: { from, to, insert: "" },
            selection: { anchor: from },
            scrollIntoView: true,
          });
        }
      }
      editorContextMenuPosition = null;
      editorContextMenuError = "";
      view.focus();
    } catch {
      editorContextMenuError = action === "paste" ? "无法读取剪贴板" : "无法写入剪贴板";
    }
  }

  onMount(() => {
    let cancelled = false;
    window.addEventListener("pointerdown", handleOutsidePointerDown);

    void Promise.all([
      import("@codemirror/state"),
      import("@codemirror/view"),
      import("@codemirror/lang-markdown"),
      import("@codemirror/language"),
      import("@lezer/highlight"),
      import("codemirror"),
      import("@codemirror/search"),
      import("@codemirror/commands"),
    ])
      .then(([state, view, markdown, language, highlight, codemirror, search, commands]) => {
        if (cancelled || !host) return;
        historyCommands = commands;
        searchPanelOpener = search.openSearchPanel;

        const syntaxTheme = language.syntaxHighlighting(
          language.HighlightStyle.define([
            { tag: highlight.tags.meta, color: "var(--code-comment)" },
            { tag: highlight.tags.keyword, color: "var(--code-keyword)" },
            {
              tag: [
                highlight.tags.atom,
                highlight.tags.bool,
                highlight.tags.url,
                highlight.tags.contentSeparator,
                highlight.tags.labelName,
              ],
              color: "var(--code-title)",
            },
            {
              tag: [highlight.tags.literal, highlight.tags.inserted],
              color: "var(--code-number)",
            },
            {
              tag: [highlight.tags.string, highlight.tags.deleted],
              color: "var(--code-string)",
            },
            {
              tag: [
                highlight.tags.regexp,
                highlight.tags.escape,
                highlight.tags.special(highlight.tags.string),
              ],
              color: "var(--code-number)",
            },
            {
              tag: highlight.tags.definition(highlight.tags.variableName),
              color: "var(--code-title)",
            },
            {
              tag: highlight.tags.local(highlight.tags.variableName),
              color: "var(--text)",
            },
            {
              tag: [highlight.tags.typeName, highlight.tags.namespace],
              color: "var(--code-string)",
            },
            { tag: highlight.tags.className, color: "var(--code-title)" },
            {
              tag: [
                highlight.tags.special(highlight.tags.variableName),
                highlight.tags.macroName,
              ],
              color: "var(--code-keyword)",
            },
            {
              tag: highlight.tags.definition(highlight.tags.propertyName),
              color: "var(--code-number)",
            },
            { tag: highlight.tags.comment, color: "var(--code-comment)" },
            {
              tag: highlight.tags.heading,
              color: "var(--code-title)",
              textDecoration: "underline",
              fontWeight: "bold",
            },
            { tag: highlight.tags.invalid, color: "var(--code-number)" },
          ]),
        );
        const searchExtension = search.search({
          top: true,
          literal: true,
          createPanel: (searchView) => {
            const dom = document.createElement("div");
            dom.className = "ml-editor-search-panel";
            dom.setAttribute("role", "search");

            const findRow = document.createElement("div");
            findRow.className = "ml-editor-search-row";
            const searchInput = document.createElement("input");
            searchInput.className = "ml-editor-search-input";
            searchInput.type = "text";
            searchInput.placeholder = "查找当前文档";
            searchInput.setAttribute("aria-label", "查找当前文档");
            searchInput.setAttribute("main-field", "true");

            const matchCount = document.createElement("span");
            matchCount.className = "ml-editor-search-count";
            matchCount.setAttribute("aria-live", "polite");
            matchCount.setAttribute("aria-atomic", "true");

            const makeButton = (
              label: string,
              accessibleName: string,
              onClick: () => void,
              className = "ml-editor-search-button",
              icon?: SearchButtonIcon,
            ) => {
              const button = document.createElement("button");
              button.type = "button";
              button.className = className;
              if (icon) button.append(createSearchIcon(icon));
              else button.textContent = label;
              button.setAttribute("aria-label", accessibleName);
              button.title = accessibleName;
              button.addEventListener("click", onClick);
              return button;
            };

            const findPrevious = makeButton("", "上一个匹配项", () => {
              search.findPrevious(searchView);
            }, "ml-editor-search-button", "previous");
            const findNext = makeButton("", "下一个匹配项", () => {
              search.findNext(searchView);
            }, "ml-editor-search-button", "next");
            const replaceToggle = makeButton("替换", "显示替换", () => {
              replaceRow.hidden = !replaceRow.hidden;
              replaceToggle.setAttribute("aria-expanded", String(!replaceRow.hidden));
              if (!replaceRow.hidden) replaceInput.focus();
            }, "ml-editor-search-toggle");
            replaceToggle.setAttribute("aria-expanded", "false");
            const optionsToggle = makeButton("选项", "显示查找选项", () => {
              optionsRow.hidden = !optionsRow.hidden;
              optionsToggle.setAttribute("aria-expanded", String(!optionsRow.hidden));
            }, "ml-editor-search-toggle");
            optionsToggle.setAttribute("aria-expanded", "false");
            const closeEditorSearchPanel = () => {
              const selection = searchView.state.selection.main;
              selectionComesFromSearch = false;
              if (!selection.empty) {
                searchView.dispatch({ selection: { anchor: selection.head } });
              }
              search.closeSearchPanel(searchView);
              searchView.focus();
            };
            const closeSearch = makeButton("", "关闭查找", closeEditorSearchPanel,
              "ml-editor-search-button", "close");

            const replaceRow = document.createElement("div");
            replaceRow.className = "ml-editor-search-row ml-editor-search-replace";
            replaceRow.hidden = true;
            const replaceInput = document.createElement("input");
            replaceInput.className = "ml-editor-search-input";
            replaceInput.type = "text";
            replaceInput.placeholder = "替换为";
            replaceInput.setAttribute("aria-label", "替换为");
            const replaceCurrent = makeButton("替换", "替换当前匹配项", () => {
              updateSearchQuery();
              search.replaceNext(searchView);
              searchInput.focus();
            }, "ml-editor-search-action");
            const replaceAll = makeButton("全部替换", "替换全部匹配项", () => {
              updateSearchQuery();
              search.replaceAll(searchView);
              searchInput.focus();
            }, "ml-editor-search-action");
            replaceRow.append(replaceInput, replaceCurrent, replaceAll);

            const optionsRow = document.createElement("div");
            optionsRow.className = "ml-editor-search-options";
            optionsRow.hidden = true;
            const caseSensitive = document.createElement("input");
            caseSensitive.type = "checkbox";
            const regexp = document.createElement("input");
            regexp.type = "checkbox";
            const wholeWord = document.createElement("input");
            wholeWord.type = "checkbox";
            const makeOption = (label: string, input: HTMLInputElement) => {
              const wrapper = document.createElement("label");
              wrapper.className = "ml-editor-search-option";
              const text = document.createElement("span");
              text.textContent = label;
              wrapper.append(input, text);
              input.addEventListener("change", () => updateSearchQuery(true));
              return wrapper;
            };
            optionsRow.append(
              makeOption("区分大小写", caseSensitive),
              makeOption("正则表达式", regexp),
              makeOption("全词匹配", wholeWord),
              makeButton("全选匹配", "选中全部匹配项", () => {
                search.selectMatches(searchView);
              }, "ml-editor-search-action"),
            );

            function updateSearchQuery(moveToMatch = false) {
              const query = new search.SearchQuery({
                search: searchInput.value,
                replace: replaceInput.value,
                caseSensitive: caseSensitive.checked,
                literal: true,
                regexp: regexp.checked,
                wholeWord: wholeWord.checked,
              });
              searchView.dispatch({
                effects: search.setSearchQuery.of(query),
              });
              if (moveToMatch && query.valid) search.findNext(searchView);
            }

            let countFrame: number | undefined;
            function updateMatchCount() {
              if (countFrame !== undefined) cancelAnimationFrame(countFrame);
              countFrame = requestAnimationFrame(() => {
                countFrame = undefined;
                const state = searchView.state;
                const query = search.getSearchQuery(state);
                if (!query.valid) {
                  matchCount.textContent = "0 条";
                  return;
                }
                let total = 0;
                const cursor = query.getCursor(state);
                let match = cursor.next();
                while (!match.done) {
                  total += 1;
                  match = cursor.next();
                }
                matchCount.textContent = total ? `${total} 条` : "0 条";
              });
            }

            searchInput.addEventListener("input", () => updateSearchQuery(true));
            replaceInput.addEventListener("input", () => updateSearchQuery());
            dom.addEventListener("keydown", (event) => {
              if (event.key === "Escape") {
                event.preventDefault();
                closeEditorSearchPanel();
              } else if (event.target === searchInput && event.key === "Enter") {
                event.preventDefault();
                if (event.shiftKey) search.findPrevious(searchView);
                else search.findNext(searchView);
              } else if (event.target === replaceInput && event.key === "Enter") {
                event.preventDefault();
                search.replaceNext(searchView);
              }
            });

            findRow.append(
              searchInput,
              matchCount,
              findPrevious,
              findNext,
              replaceToggle,
              optionsToggle,
              closeSearch,
            );
            dom.append(findRow, replaceRow, optionsRow);

            return {
              dom,
              mount() {
                const query = search.getSearchQuery(searchView.state);
                searchInput.value = query.search;
                replaceInput.value = query.replace;
                caseSensitive.checked = query.caseSensitive;
                regexp.checked = query.regexp;
                wholeWord.checked = query.wholeWord;
                searchInput.focus();
                searchInput.select();
                updateMatchCount();
              },
              update(update) {
                const query = search.getSearchQuery(update.state);
                if (searchInput.value !== query.search) searchInput.value = query.search;
                if (replaceInput.value !== query.replace) replaceInput.value = query.replace;
                caseSensitive.checked = query.caseSensitive;
                regexp.checked = query.regexp;
                wholeWord.checked = query.wholeWord;
                if (update.docChanged || update.transactions.some((transaction) =>
                  transaction.effects.some((effect) => effect.is(search.setSearchQuery))
                )) {
                  updateMatchCount();
                }
              },
              destroy() {
                if (countFrame !== undefined) cancelAnimationFrame(countFrame);
              },
            };
          },
        });
        const compartments: EditorCompartments = editorCompartments ?? {
          lineWrapping: new state.Compartment(),
          indentation: new state.Compartment(),
        };
        mountedCompartments = compartments;
        const extensions = [
          state.Prec.highest(view.keymap.of([
            { key: "Mod-s", run: () => { onSave(false); return true; } },
            { key: "Mod-Shift-s", run: () => { onSave(true); return true; } },
            { key: "Mod-b", run: () => applyMarkdownInsertion("bold") },
            { key: "Mod-i", run: () => applyMarkdownInsertion("italic") },
            { key: "Mod-k", run: () => applyMarkdownInsertion("link") },
            { key: "Mod-Shift-k", run: () => applyMarkdownInsertion("inlineCode") },
            { key: "Mod-Shift-c", run: () => applyMarkdownInsertion("codeBlock") },
          ])),
          codemirror.basicSetup,
          searchExtension,
          markdown.markdown(),
          syntaxTheme,
          state.EditorState.phrases.of({
            Find: "查找",
            Replace: "替换",
            next: "下一个",
            previous: "上一个",
            all: "选中全部匹配项",
            "match case": "区分大小写",
            regexp: "正则表达式",
            "by word": "全词匹配",
            replace: "替换",
            "replace all": "全部替换",
            close: "关闭",
            "Go to line": "跳转到行",
            go: "跳转",
            "replaced match on line $": "已替换第 $1 行的匹配项",
            "replaced $ matches": "已替换 $1 个匹配项",
            "current match": "当前匹配项",
            "on line": "所在行",
          }),
          compartments.lineWrapping.of(
            wordWrap ? view.EditorView.lineWrapping : [],
          ),
          compartments.indentation.of([
            language.indentUnit.of(" ".repeat(indentWidth)),
            state.EditorState.tabSize.of(indentWidth),
          ]),
          state.EditorState.lineSeparator.of(lineEnding),
          view.EditorView.updateListener.of((update) => {
            if (
              update.docChanged ||
              update.selectionSet ||
              update.transactions.some((transaction) => transaction.effects.length > 0)
            ) {
              onEditorStateChange(update.state, compartments, update.docChanged);
            }
            if (update.docChanged || update.selectionSet || update.focusChanged) {
              if (update.docChanged) selectionComesFromSearch = false;
              else if (update.selectionSet) {
                selectionComesFromSearch = update.transactions.some((transaction) =>
                  transaction.isUserEvent("select.search"),
                );
              }
              updateSelectionToolbar(update.state);
            }
            if (update.docChanged || update.selectionSet) {
              const cursor = update.state.selection.main.head;
              onCursorChange(cursor, update.state.doc.lineAt(cursor).number - 1);
            }
          }),
        ];

        editorView = new view.EditorView({
          state:
            editorState ??
            state.EditorState.create({
              doc: value,
              selection: { anchor: Math.min(cursor, value.length) },
              extensions,
            }),
          parent: host,
        });
        editorView.dom.addEventListener("pointerdown", handleEditorPointerDown);
        editorView.dom.addEventListener("pointerup", handleEditorPointerUp);
        editorView.dom.addEventListener("keyup", handleEditorKeyUp);
        editorView.dom.addEventListener("contextmenu", handleEditorContextMenu);
        editorView.dom.addEventListener("focusout", handleEditorFocusOut);
        applyEditorPreferences = (nextWordWrap, nextIndentWidth) => {
          if (!editorView) return;
          editorView.dispatch({
            effects: [
              compartments.lineWrapping.reconfigure(
                nextWordWrap ? view.EditorView.lineWrapping : [],
              ),
              compartments.indentation.reconfigure([
                language.indentUnit.of(" ".repeat(nextIndentWidth)),
                state.EditorState.tabSize.of(nextIndentWidth),
              ]),
            ],
          });
          handleEditorScroll();
        };
        applyEditorPreferences(wordWrap, indentWidth);
        editorLoading = false;
        onEditorStateChange(editorView.state, compartments, false);
        editorView.focus();
        const maxScrollTop = editorView.scrollDOM.scrollHeight - editorView.scrollDOM.clientHeight;
        editorView.scrollDOM.scrollTop = Math.max(0, maxScrollTop) * editorScrollProgress;
        const initialCursor = editorView.state.selection.main.head;
        onCursorChange(initialCursor, editorView.state.doc.lineAt(initialCursor).number - 1);
        if (searchRequested) {
          searchRequested = false;
          search.openSearchPanel(editorView);
        }
        if (pendingInsertAction) {
          const action = pendingInsertAction;
          pendingInsertAction = null;
          applyMarkdownInsertion(action);
        }
        editorView.scrollDOM.addEventListener("scroll", handleEditorScroll, { passive: true });
        editorResizeObserver = new ResizeObserver(handleEditorScroll);
        editorResizeObserver.observe(editorView.dom);
        handleEditorScroll();
      })
      .catch(() => {
        if (cancelled) return;
        editorLoading = false;
        loadingError = "编辑器加载失败，请切换到阅读模式后重试。";
      });

    return () => {
      cancelled = true;
      window.removeEventListener("pointerdown", handleOutsidePointerDown);
      if (editorView) {
        if (scrollFrame !== undefined) cancelAnimationFrame(scrollFrame);
        scrollFrame = undefined;
        const cursor = editorView.state.selection.main.head;
        onCursorChange(cursor, editorView.state.doc.lineAt(cursor).number - 1);
        onScrollPositionChange(editorView.scrollDOM.scrollTop, scrollProgress());
        if (mountedCompartments) {
          onEditorStateChange(editorView.state, mountedCompartments, false);
        }
        editorResizeObserver?.disconnect();
        editorResizeObserver = undefined;
        editorView.dom.removeEventListener("pointerdown", handleEditorPointerDown);
        editorView.dom.removeEventListener("pointerup", handleEditorPointerUp);
        editorView.dom.removeEventListener("keyup", handleEditorKeyUp);
        editorView.dom.removeEventListener("contextmenu", handleEditorContextMenu);
        editorView.dom.removeEventListener("focusout", handleEditorFocusOut);
        editorView.scrollDOM.removeEventListener("scroll", handleEditorScroll);
        editorView.destroy();
        editorView = undefined;
        mountedCompartments = undefined;
        applyEditorPreferences = undefined;
      }
      searchRequested = false;
      pendingInsertAction = null;
      searchPanelOpener = undefined;
      historyCommands = undefined;
      selectionToolbarPosition = null;
      editorContextMenuPosition = null;
    };
  });

  function applyMarkdownHeading(level: HeadingLevel): boolean {
    if (!editorView) return false;
    const state = editorView.state;
    const selection = state.selection.main;
    const firstLine = state.doc.lineAt(selection.from);
    const lastLine = state.doc.lineAt(selection.to);
    const lastLineNumber =
      selection.empty || selection.to > lastLine.from
        ? lastLine.number
        : Math.max(firstLine.number, lastLine.number - 1);
    const changes = [];

    for (let lineNumber = firstLine.number; lineNumber <= lastLineNumber; lineNumber += 1) {
      const line = state.doc.line(lineNumber);
      const existingHeading = /^( {0,3})(#{1,6})(?:[ \t]+|$)/.exec(line.text);
      const indentLength = existingHeading?.[1].length ?? 0;
      const from = line.from + indentLength;
      const to = existingHeading ? line.from + existingHeading[0].length : from;
      changes.push({ from, to, insert: `${"#".repeat(level)} ` });
    }

    editorView.dispatch({ changes, scrollIntoView: true });
    editorView.focus();
    selectionHeadingMenuOpen = false;
    selectionHeadingMenuAbove = false;
    contextHeadingMenuOpen = false;
    updateSelectionToolbar(editorView.state);
    return true;
  }

  function applyMarkdownInsertion(action: InsertAction): boolean {
    if (typeof action === "object") return applyMarkdownHeading(action.level);
    if (!editorView) return false;
    const state = editorView.state;
    const { from, to } = state.selection.main;
    const selected = state.sliceDoc(from, to);
    let prefix: string;
    let suffix: string;
    let content: string;
    let insertion: string;
    let contentStart: number;

    if (action === "codeBlock") {
      prefix = from > 0 && state.sliceDoc(from - 1, from) !== "\n" ? "\n" : "";
      suffix = to < state.doc.length && state.sliceDoc(to, to + 1) !== "\n" ? "\n" : "";
      content = selected || "代码";
      const contentEnd = content.endsWith("\n") ? "" : "\n";
      insertion = `${prefix}\`\`\`\n${content}${contentEnd}\`\`\`${suffix}`;
      contentStart = from + prefix.length + 4;
    } else {
      const templates = {
        bold: ["**", "**", "粗体文字"],
        italic: ["*", "*", "斜体文字"],
        inlineCode: ["`", "`", "代码"],
        link: ["[", "](链接地址)", "链接文字"],
      } satisfies Record<WrappedInsertAction, [string, string, string]>;
      [prefix, suffix, content] = templates[action];
      content = selected || content;
      insertion = `${prefix}${content}${suffix}`;
      contentStart = from + prefix.length;
    }

    editorView.dispatch({
      changes: { from, to, insert: insertion },
      selection: { anchor: contentStart, head: contentStart + content.length },
      scrollIntoView: true,
    });
    editorView.focus();
    selectionHeadingMenuOpen = false;
    selectionHeadingMenuAbove = false;
    contextHeadingMenuOpen = false;
    updateSelectionToolbar(editorView.state);
    return true;
  }

  export function scrollToLine(line: number) {
    if (!editorView) return;
    const lineNumber = Math.min(
      Math.max(1, Math.trunc(line) + 1),
      editorView.state.doc.lines,
    );
    const position = editorView.state.doc.line(lineNumber).from;
    editorView.dispatch({
      selection: { anchor: position },
      scrollIntoView: true,
    });
    editorView.focus();
  }

  export function scrollToProgress(progress: number) {
    if (!editorView) return;
    const scrollDOM = editorView.scrollDOM;
    const maxScrollTop = Math.max(0, scrollDOM.scrollHeight - scrollDOM.clientHeight);
    const nextScrollTop = maxScrollTop * Math.min(1, Math.max(0, progress));
    if (Math.abs(scrollDOM.scrollTop - nextScrollTop) > 2) {
      scrollDOM.scrollTop = nextScrollTop;
    }
  }

  export function focusEditor() {
    editorView?.focus();
  }

  export function openSearch() {
    if (!editorView || !searchPanelOpener) {
      searchRequested = true;
      return;
    }
    searchPanelOpener(editorView);
  }

  export function insertMarkdown(action: InsertAction) {
    if (!editorView) {
      pendingInsertAction = action;
      return;
    }
    applyMarkdownInsertion(action);
  }
</script>

<div class="editor-shell" class:dark={darkMode} bind:this={shell}>
  {#if loadingError}
    <div class="editor-error" role="alert">{loadingError}</div>
  {:else}
    <div class="editor-host" bind:this={host}></div>
    {#if editorLoading}
      <div class="editor-loading" role="status">正在加载编辑器…</div>
    {/if}
    {#if selectionToolbarPosition}
      <div
        class="selection-toolbar"
        class:below={selectionToolbarBelow}
        class:side-left={selectionToolbarPosition.placement === "left"}
        class:side-right={selectionToolbarPosition.placement === "right"}
        bind:this={selectionToolbarElement}
        role="toolbar"
        aria-label="文字格式"
        tabindex="-1"
        style={`left:${selectionToolbarPosition.left}px;top:${selectionToolbarPosition.top}px`}
        in:popupTransition={{ duration: 125, start: 0.98 }}
        onpointerdown={(event) => event.preventDefault()}
      >
        <button type="button" aria-label="粗体" title="粗体" onclick={() => applyMarkdownInsertion("bold")}>
          <Bold size={16} />
        </button>
        <button type="button" aria-label="斜体" title="斜体" onclick={() => applyMarkdownInsertion("italic")}>
          <Italic size={16} />
        </button>
        <button type="button" aria-label="行内代码" title="行内代码" onclick={() => applyMarkdownInsertion("inlineCode")}>
          <Code size={16} />
        </button>
        <button type="button" aria-label="链接" title="链接" onclick={() => applyMarkdownInsertion("link")}>
          <Link size={16} />
        </button>
        <div class="selection-heading-control">
          <button
            type="button"
            class="selection-heading-trigger"
            aria-label="标题级别"
            title="标题级别"
            aria-expanded={selectionHeadingMenuOpen}
            aria-haspopup="true"
            aria-controls="selection-heading-options"
            onclick={toggleSelectionHeadingMenu}
          >
            <Heading1 size={16} /><ChevronDown size={11} />
          </button>
          {#if selectionHeadingMenuOpen}
            <div
              id="selection-heading-options"
              class="selection-heading-options"
              class:above={selectionHeadingMenuAbove}
              role="group"
              aria-label="标题级别"
              transition:popupTransition={{ duration: 125, start: 0.98 }}
            >
              {#each headingLevels as level}
                <button
                  type="button"
                  aria-label={`${level} 级标题`}
                  title={`${level} 级标题`}
                  onclick={() => applyMarkdownInsertion({ type: "heading", level })}
                >H{level}</button>
              {/each}
            </div>
          {/if}
        </div>
      </div>
    {/if}
    {#if editorContextMenuPosition}
      <div
        class="editor-context-menu"
        bind:this={contextMenuElement}
        role="menu"
        aria-label="编辑操作"
        tabindex="-1"
        style={`left:${editorContextMenuPosition.left}px;top:${editorContextMenuPosition.top}px`}
        transition:popupTransition={{ duration: 150, start: 0.97 }}
        onkeydown={handleContextMenuKeydown}
      >
        <button type="button" role="menuitem" disabled={!contextMenuCanUndo} onclick={() => runHistoryCommand("undo")}>
          <Undo2 size={16} /><span>撤销</span><kbd>Ctrl+Z</kbd>
        </button>
        <button type="button" role="menuitem" disabled={!contextMenuCanRedo} onclick={() => runHistoryCommand("redo")}>
          <Redo2 size={16} /><span>重做</span><kbd>Ctrl+Y</kbd>
        </button>
        <div class="context-menu-divider"></div>
        <button type="button" role="menuitem" disabled={!contextMenuHasSelection} onclick={() => void runClipboardAction("cut")}>
          <Scissors size={16} /><span>剪切</span><kbd>Ctrl+X</kbd>
        </button>
        <button type="button" role="menuitem" disabled={!contextMenuHasSelection} onclick={() => void runClipboardAction("copy")}>
          <Copy size={16} /><span>复制</span><kbd>Ctrl+C</kbd>
        </button>
        <button type="button" role="menuitem" onclick={() => void runClipboardAction("paste")}>
          <ClipboardPaste size={16} /><span>粘贴</span><kbd>Ctrl+V</kbd>
        </button>
        <button type="button" role="menuitem" onclick={selectAllFromContextMenu}>
          <TextSelect size={16} /><span>全选</span><kbd>Ctrl+A</kbd>
        </button>
        <div class="context-menu-divider"></div>
        <button type="button" role="menuitem" onclick={() => insertFromContextMenu("bold")}>
          <Bold size={16} /><span>粗体</span><kbd>Ctrl+B</kbd>
        </button>
        <button type="button" role="menuitem" onclick={() => insertFromContextMenu("italic")}>
          <Italic size={16} /><span>斜体</span><kbd>Ctrl+I</kbd>
        </button>
        <div class="context-heading-control">
          <button
            type="button"
            role="menuitem"
            aria-haspopup="true"
            aria-expanded={contextHeadingMenuOpen}
            aria-controls="context-heading-options"
            onclick={() => (contextHeadingMenuOpen = !contextHeadingMenuOpen)}
          >
            <Heading1 size={16} /><span>标题</span>
            {#if contextHeadingMenuOpen}<ChevronDown size={15} />{:else}<ChevronRight size={15} />{/if}
          </button>
          {#if contextHeadingMenuOpen}
            <div
              id="context-heading-options"
              class="context-heading-options"
              role="group"
              aria-label="标题级别"
              transition:popupTransition={{ duration: 125, start: 0.98 }}
            >
              {#each headingLevels as level}
                <button
                  type="button"
                  role="menuitem"
                  aria-label={`${level} 级标题`}
                  onclick={() => insertFromContextMenu({ type: "heading", level })}
                >H{level}</button>
              {/each}
            </div>
          {/if}
        </div>
        <div class="context-menu-divider"></div>
        <button type="button" role="menuitem" onclick={() => insertFromContextMenu("inlineCode")}>
          <Code size={16} /><span>行内代码</span><kbd>Ctrl+Shift+K</kbd>
        </button>
        <button type="button" role="menuitem" onclick={() => insertFromContextMenu("codeBlock")}>
          <Code size={16} /><span>代码块</span><kbd>Ctrl+Shift+C</kbd>
        </button>
        <button type="button" role="menuitem" onclick={() => insertFromContextMenu("link")}>
          <Link size={16} /><span>链接</span><kbd>Ctrl+K</kbd>
        </button>
        {#if editorContextMenuError}
          <div class="context-menu-error" role="status">{editorContextMenuError}</div>
        {/if}
      </div>
    {/if}
  {/if}
</div>

<style>
  .editor-shell,
  .editor-host {
    width: 100%;
    height: 100%;
    min-width: 0;
    min-height: 0;
  }

  .editor-shell {
    position: relative;
    overflow: hidden;
    background: var(--surface);
  }

  .editor-loading {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    color: var(--subtle);
    font-size: 15px;
    background: var(--surface);
  }

  .selection-toolbar,
  .editor-context-menu {
    position: absolute;
    z-index: 6;
    border: 1px solid var(--line-strong);
    border-radius: var(--radius-popover);
    color: var(--text);
    background: var(--surface);
    box-shadow: var(--shadow);
  }

  .selection-toolbar {
    display: flex;
    align-items: center;
    gap: 2px;
    padding: 4px;
    transform-origin: bottom center;
  }

  .selection-toolbar.below {
    transform-origin: top center;
  }

  .selection-toolbar.side-left {
    transform-origin: right center;
  }

  .selection-toolbar.side-right {
    transform-origin: left center;
  }

  .selection-toolbar button {
    display: grid;
    width: 32px;
    height: 32px;
    place-items: center;
    border: 0;
    border-radius: var(--radius-control);
    color: var(--muted);
    background: transparent;
    cursor: pointer;
  }

  .selection-toolbar button:hover,
  .editor-context-menu button:hover {
    color: var(--text);
    background: var(--hover);
  }

  .selection-heading-control {
    position: relative;
  }

  .selection-toolbar .selection-heading-trigger {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 0;
  }

  .selection-heading-options {
    position: absolute;
    top: calc(100% + 6px);
    left: 0;
    z-index: 7;
    display: grid;
    grid-template-columns: repeat(3, 32px);
    gap: 2px;
    padding: 4px;
    border: 1px solid var(--line-strong);
    border-radius: var(--radius-popover);
    background: var(--surface);
    box-shadow: var(--shadow);
    transform-origin: top left;
  }

  .selection-heading-options.above {
    top: auto;
    bottom: calc(100% + 6px);
    transform-origin: bottom left;
  }

  .selection-toolbar .selection-heading-options button {
    width: 32px;
    height: 30px;
    font-size: 15px;
    font-weight: 650;
  }

  .editor-context-menu button:disabled {
    color: var(--subtle);
    opacity: 0.6;
    cursor: default;
  }

  .editor-context-menu button:disabled:hover {
    background: transparent;
  }

  .editor-context-menu {
    display: flex;
    flex-direction: column;
    width: 230px;
    max-height: calc(100% - 16px);
    box-sizing: border-box;
    overflow: auto;
    padding: 5px;
    transform-origin: top left;
  }

  .editor-context-menu button {
    display: flex;
    align-items: center;
    flex: 0 0 auto;
    width: 100%;
    min-height: 36px;
    gap: 10px;
    padding: 0 9px;
    border: 0;
    border-radius: var(--radius-control);
    color: var(--text);
    background: transparent;
    font: inherit;
    font-size: 15px;
    text-align: left;
    cursor: pointer;
  }

  .editor-context-menu button span {
    flex: 1;
  }

  .context-heading-control {
    display: flex;
    flex: 0 0 auto;
    flex-direction: column;
  }

  .context-heading-options {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 2px;
    margin: 0 5px 4px 34px;
    padding: 3px;
    border-radius: var(--radius-control);
    background: var(--window-bg);
  }

  .editor-context-menu .context-heading-options button {
    justify-content: center;
    width: auto;
    min-height: 30px;
    padding: 0;
    font-size: 15px;
    font-weight: 650;
  }

  .editor-context-menu kbd {
    color: var(--muted);
    font: inherit;
    font-size: 15px;
  }

  .context-menu-error {
    flex: 0 0 auto;
    padding: 5px 9px 4px;
    color: var(--error);
    font-size: 15px;
  }

  .context-menu-divider {
    flex: 0 0 auto;
    height: 1px;
    margin: 4px 6px;
    background: var(--line);
  }

  .selection-toolbar button:focus-visible,
  .editor-context-menu button:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: -2px;
  }

  .editor-shell.dark {
    color-scheme: dark;
  }

  .editor-shell:not(.dark) {
    color-scheme: light;
  }

  :global(.editor-host .cm-editor) {
    height: 100%;
    color: var(--text);
    background: var(--surface);
    font-size: 16px;
  }

  :global(.editor-host .cm-scroller) {
    overflow: auto;
    font-family: Consolas, "Cascadia Code", "Microsoft YaHei UI", monospace;
    line-height: 1.65;
  }

  :global(.editor-host .cm-content) {
    padding: 28px 36px 80px;
    caret-color: var(--accent);
  }

  :global(.editor-host .cm-gutters) {
    border-right: 1px solid var(--line);
    color: var(--subtle);
    background: var(--window-bg);
  }

  :global(.editor-host .cm-activeLine),
  :global(.editor-host .cm-activeLineGutter) {
    background: color-mix(in srgb, var(--accent-soft) 42%, transparent);
  }

  :global(.editor-host .cm-cursor) {
    border-left-color: var(--accent);
  }

  :global(.editor-shell .editor-host .cm-panels) {
    color: var(--text);
    background: var(--surface);
  }

  :global(.editor-shell .editor-host .cm-editor) {
    position: relative;
  }

  :global(.editor-shell .editor-host .cm-panels) {
    position: fixed;
    top: calc(var(--titlebar-height) + var(--toolbar-height) + 6px);
    right: var(--toolbar-inline-padding, 18px);
    bottom: auto;
    left: auto;
    z-index: 30;
    width: 0;
    height: 0;
    max-width: none;
    overflow: visible;
    border: 0;
    background: transparent;
    pointer-events: none;
  }

  :global(.editor-shell .editor-host .ml-editor-search-panel) {
    position: fixed;
    top: calc(var(--titlebar-height) + var(--toolbar-height) + 6px);
    right: var(--toolbar-inline-padding, 18px);
    z-index: 30;
    display: flex;
    flex-direction: column;
    gap: 7px;
    width: min(470px, calc(100vw - 24px));
    padding: 8px;
    box-sizing: border-box;
    border: 1px solid var(--line-strong);
    border-radius: var(--radius-popover);
    color: var(--text);
    background: var(--surface);
    box-shadow: var(--shadow);
    pointer-events: auto;
  }

  :global(.editor-shell .editor-host .ml-editor-search-row) {
    display: flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
  }

  :global(.editor-shell .editor-host .ml-editor-search-input) {
    flex: 1 1 auto;
    width: 40px;
    min-width: 48px;
    height: 32px;
    padding: 0 9px;
    box-sizing: border-box;
    border: 1px solid var(--line);
    border-radius: var(--radius-control);
    outline: 0;
    color: var(--text);
    background: var(--surface-muted);
    font: inherit;
    font-size: 15px;
  }

  :global(.editor-shell .editor-host .ml-editor-search-input:focus) {
    border-color: var(--accent);
    box-shadow: 0 0 0 2px var(--accent-soft);
  }

  :global(.editor-shell .editor-host .ml-editor-search-input::placeholder) {
    color: var(--subtle);
  }

  :global(.editor-shell .editor-host .ml-editor-search-count) {
    flex: 0 0 auto;
    min-width: 42px;
    color: var(--subtle);
    font-size: 15px;
    text-align: center;
    white-space: nowrap;
  }

  :global(.editor-shell .editor-host .ml-editor-search-button),
  :global(.editor-shell .editor-host .ml-editor-search-toggle),
  :global(.editor-shell .editor-host .ml-editor-search-action) {
    display: inline-flex;
    flex: 0 0 auto;
    align-items: center;
    justify-content: center;
    height: 28px;
    padding: 0;
    border: 0;
    border-radius: var(--radius-control);
    color: var(--muted);
    background: transparent;
    font: inherit;
    cursor: pointer;
  }

  :global(.editor-shell .editor-host .ml-editor-search-button) {
    width: 28px;
  }

  :global(.editor-shell .editor-host .ml-editor-search-button svg) {
    flex: 0 0 auto;
  }

  :global(.editor-shell .editor-host .ml-editor-search-toggle),
  :global(.editor-shell .editor-host .ml-editor-search-action) {
    padding: 0 7px;
    font-size: 15px;
  }

  :global(.editor-shell .editor-host .ml-editor-search-button:hover),
  :global(.editor-shell .editor-host .ml-editor-search-toggle:hover),
  :global(.editor-shell .editor-host .ml-editor-search-action:hover) {
    color: var(--text);
    background: var(--hover);
  }

  :global(.editor-shell .editor-host .ml-editor-search-button:focus-visible),
  :global(.editor-shell .editor-host .ml-editor-search-toggle:focus-visible),
  :global(.editor-shell .editor-host .ml-editor-search-action:focus-visible) {
    outline: 2px solid var(--accent);
    outline-offset: 1px;
  }

  :global(.editor-shell .editor-host .ml-editor-search-replace) {
    padding-top: 7px;
    border-top: 1px solid var(--line);
  }

  :global(.editor-shell .editor-host .ml-editor-search-options) {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px 12px;
    padding: 2px 1px;
  }

  :global(.editor-shell .editor-host .ml-editor-search-option) {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    color: var(--muted);
    font-size: 15px;
    white-space: nowrap;
    cursor: pointer;
  }

  :global(.editor-shell .editor-host .ml-editor-search-option input) {
    margin: 0;
    accent-color: var(--accent);
  }

  :global(.editor-shell .editor-host .ml-editor-search-replace[hidden]),
  :global(.editor-shell .editor-host .ml-editor-search-options[hidden]) {
    display: none;
  }

  :global(.editor-shell .editor-host .cm-searchMatch) {
    background-color: color-mix(in srgb, var(--accent) 22%, transparent);
  }

  :global(.editor-shell .editor-host .cm-searchMatch-selected) {
    background-color: color-mix(in srgb, var(--accent) 46%, transparent);
  }

  .editor-error {
    padding: 24px;
    color: var(--text);
  }
</style>
