"use client";

import {
  BoldIcon,
  CodeIcon,
  CrossReferenceIcon,
  HeadingIcon,
  ItalicIcon,
  LinkIcon,
  ListOrderedIcon,
  ListUnorderedIcon,
  MarkdownIcon,
  MentionIcon,
  PaperclipIcon,
  QuoteIcon,
  TasklistIcon,
} from "@primer/octicons-react";
import { type KeyboardEvent, type ReactNode, useEffect, useImperativeHandle, useRef, useState } from "react";
import useSWR from "swr";
import { MarkdownBody } from "@/components/markdown/markdown-body";
import { activeTrigger, applyFormat, caretPosition, type Format } from "@/components/markdown/textarea-tools";
import { Avatar } from "@/components/ui/avatar";
import { IssueStateIcon } from "@/components/ui/misc";
import { api, asApiError, type IssueList, upload } from "@/lib/api";
import { cn } from "@/lib/utils";

const TOOLS: { format: Format; label: string; icon: ReactNode; gapBefore?: boolean }[] = [
  { format: "heading", label: "Heading", icon: <HeadingIcon size={16} /> },
  { format: "bold", label: "Bold", icon: <BoldIcon size={16} /> },
  { format: "italic", label: "Italic", icon: <ItalicIcon size={16} /> },
  { format: "quote", label: "Quote", icon: <QuoteIcon size={16} /> },
  { format: "code", label: "Code", icon: <CodeIcon size={16} /> },
  { format: "link", label: "Link", icon: <LinkIcon size={16} /> },
  { format: "ol", label: "Numbered list", icon: <ListOrderedIcon size={16} />, gapBefore: true },
  { format: "ul", label: "Unordered list", icon: <ListUnorderedIcon size={16} /> },
  { format: "task", label: "Task list", icon: <TasklistIcon size={16} /> },
  { format: "mention", label: "Mention", icon: <MentionIcon size={16} />, gapBefore: true },
  { format: "reference", label: "Reference", icon: <CrossReferenceIcon size={16} /> },
];

type Suggestion = { key: string; insert: string; node: ReactNode };

export type EditorHandle = { focus: () => void; insert: (text: string) => void };

export function MarkdownEditor({
  value,
  onChange,
  repo,
  placeholder = "Use Markdown to format your comment",
  minHeight = 100,
  autoFocus,
  onSubmit,
  id,
  handle,
}: {
  value: string;
  onChange: (value: string) => void;
  /** `owner/name`: enables `#` and `@` suggestions and resolves them in the preview. */
  repo: string;
  placeholder?: string;
  minHeight?: number;
  autoFocus?: boolean;
  /** Ctrl/Cmd+Enter. */
  onSubmit?: () => void;
  id?: string;
  handle?: React.Ref<EditorHandle>;
}) {
  const [tab, setTab] = useState<"write" | "preview">("write");
  const area = useRef<HTMLTextAreaElement>(null);
  const pendingSelection = useRef<[number, number] | null>(null);
  const [trigger, setTrigger] = useState<{ char: "@" | "#"; query: string; from: number } | null>(null);
  const [active, setActive] = useState(0);
  const [popup, setPopup] = useState({ top: 0, left: 0 });
  const [uploading, setUploading] = useState(0);
  const [uploadError, setUploadError] = useState<string | null>(null);
  const [dragging, setDragging] = useState(false);
  const fileInput = useRef<HTMLInputElement>(null);
  // Uploads finish later, so they must write into the latest text, not the text at drop time.
  const latest = useRef(value);
  latest.current = value;

  async function addFiles(files: File[]) {
    if (files.length === 0) return;
    setUploadError(null);
    for (const file of files) {
      const placeholder = `![Uploading ${file.name || "file"}…]()`;
      const el = area.current;
      const at = el ? el.selectionStart : latest.current.length;
      const before = latest.current.slice(0, at);
      const lead = before && !before.endsWith("\n") ? "\n" : "";
      const next = `${before}${lead}${placeholder}\n${latest.current.slice(at)}`;
      latest.current = next;
      onChange(next);
      setUploading((n) => n + 1);
      try {
        const done = await upload(repo, file);
        latest.current = latest.current.replace(placeholder, done.markdown);
      } catch (err) {
        latest.current = latest.current.replace(`${placeholder}\n`, "").replace(placeholder, "");
        setUploadError(`${file.name || "File"}: ${asApiError(err).summary}`);
      }
      onChange(latest.current);
      setUploading((n) => n - 1);
    }
  }

  // Restore the selection after React re-renders the controlled textarea.
  useEffect(() => {
    if (pendingSelection.current && area.current) {
      area.current.focus();
      area.current.setSelectionRange(...pendingSelection.current);
      pendingSelection.current = null;
    }
  });

  useImperativeHandle(handle, () => ({
    focus: () => area.current?.focus(),
    insert: (text: string) => {
      const next = value ? `${value.replace(/\n*$/, "")}\n\n${text}` : text;
      pendingSelection.current = [next.length, next.length];
      setTab("write");
      onChange(next);
    },
  }));

  const preview = useSWR(tab === "preview" && value.trim() ? ["/markdown", value, repo] : null, () =>
    api<{ html: string }>("/markdown", { method: "POST", body: { text: value, repo } }),
  );

  const users = useSWR<{ login: string }[]>(trigger?.char === "@" ? `/repos/${repo}/mentionables?q=${encodeURIComponent(trigger.query)}` : null, api);
  const issues = useSWR<IssueList>(
    trigger?.char === "#" ? `/repos/${repo}/issues?state=all&per_page=6&q=${encodeURIComponent(trigger.query)}` : null,
    api,
  );
  const suggestions: Suggestion[] =
    trigger?.char === "@"
      ? (users.data ?? []).map((u) => ({
          key: u.login,
          insert: `@${u.login} `,
          node: (
            <>
              <Avatar login={u.login} size={20} />
              <span className="font-semibold">{u.login}</span>
            </>
          ),
        }))
      : trigger?.char === "#"
        ? (issues.data?.items ?? []).map((i) => ({
            key: String(i.number),
            insert: `#${i.number} `,
            node: (
              <>
                <IssueStateIcon issue={i} />
                <span className="min-w-0 flex-1 truncate">{i.title}</span>
                <span className="text-fg-muted">#{i.number}</span>
              </>
            ),
          }))
        : [];
  const open = !!trigger && suggestions.length > 0;

  function refreshTrigger(el: HTMLTextAreaElement) {
    const next = el.selectionStart === el.selectionEnd ? activeTrigger(el.value, el.selectionStart) : null;
    setTrigger(next);
    setActive(0);
    if (next) {
      const pos = caretPosition(el, next.from);
      setPopup({ top: pos.top + pos.height + 4, left: Math.min(pos.left, Math.max(0, el.clientWidth - 280)) });
    }
  }

  function choose(s: Suggestion) {
    const el = area.current;
    if (!el || !trigger) return;
    const next = value.slice(0, trigger.from) + s.insert + value.slice(el.selectionStart);
    const caret = trigger.from + s.insert.length;
    pendingSelection.current = [caret, caret];
    setTrigger(null);
    onChange(next);
  }

  function format(kind: Format) {
    const el = area.current;
    if (!el) return;
    const result = applyFormat(value, el.selectionStart, el.selectionEnd, kind);
    pendingSelection.current = [result.start, result.end];
    onChange(result.value);
    if (kind === "mention" || kind === "reference") {
      setTrigger({ char: kind === "mention" ? "@" : "#", query: "", from: result.start - 1 });
      setActive(0);
      requestAnimationFrame(() => area.current && refreshTrigger(area.current));
    }
  }

  function onKeyDown(e: KeyboardEvent<HTMLTextAreaElement>) {
    if (open) {
      if (e.key === "ArrowDown" || e.key === "ArrowUp") {
        e.preventDefault();
        setActive((a) => (a + (e.key === "ArrowDown" ? 1 : -1) + suggestions.length) % suggestions.length);
        return;
      }
      if (e.key === "Enter" || e.key === "Tab") {
        e.preventDefault();
        choose(suggestions[active]);
        return;
      }
      if (e.key === "Escape") {
        e.preventDefault();
        setTrigger(null);
        return;
      }
    }
    const mod = e.metaKey || e.ctrlKey;
    if (mod && e.key === "Enter") {
      e.preventDefault();
      onSubmit?.();
    } else if (mod && !e.shiftKey && ["b", "i", "k", "e"].includes(e.key.toLowerCase())) {
      e.preventDefault();
      format(({ b: "bold", i: "italic", k: "link", e: "code" } as const)[e.key.toLowerCase() as "b" | "i" | "k" | "e"]);
    }
  }

  const tabClass = (on: boolean) =>
    cn(
      "-mb-px h-10 cursor-pointer rounded-t-md border border-b-0 px-4 text-sm",
      on ? "border-line bg-canvas text-fg" : "border-transparent text-fg-muted hover:text-fg",
    );

  return (
    <div className="rounded-md border border-line bg-canvas focus-within:border-fg-accent focus-within:shadow-[0_0_0_1px_var(--fgColor-accent)]">
      <div className="flex items-end gap-2 rounded-t-md border-b border-line bg-canvas-muted pr-2">
        <div role="tablist" className="-ml-px flex">
          <button type="button" role="tab" aria-selected={tab === "write"} className={tabClass(tab === "write")} onClick={() => setTab("write")}>
            Write
          </button>
          <button type="button" role="tab" aria-selected={tab === "preview"} className={tabClass(tab === "preview")} onClick={() => setTab("preview")}>
            Preview
          </button>
        </div>
        {tab === "write" && (
          <div role="toolbar" aria-label="Formatting tools" className="ml-auto flex h-10 min-w-0 items-center overflow-x-auto [scrollbar-width:none]">
            {TOOLS.map((t) => (
              <button
                key={t.format}
                type="button"
                aria-label={t.label}
                title={t.label}
                onMouseDown={(e) => e.preventDefault()}
                onClick={() => format(t.format)}
                className={cn(
                  "flex h-7 w-7 shrink-0 cursor-pointer items-center justify-center rounded-md text-fg-muted hover:bg-control-transparent-hover",
                  t.gapBefore && "ml-2",
                )}
              >
                {t.icon}
              </button>
            ))}
          </div>
        )}
      </div>

      {tab === "write" ? (
        <div className="relative">
          <textarea
            id={id}
            ref={area}
            value={value}
            autoFocus={autoFocus}
            placeholder={placeholder}
            style={{ minHeight }}
            onChange={(e) => {
              onChange(e.target.value);
              refreshTrigger(e.target);
            }}
            onKeyDown={onKeyDown}
            onClick={(e) => refreshTrigger(e.currentTarget)}
            onBlur={() => setTimeout(() => setTrigger(null), 150)}
            onPaste={(e) => {
              const files = [...e.clipboardData.files];
              if (files.length > 0) {
                e.preventDefault();
                addFiles(files);
              }
            }}
            onDragOver={(e) => {
              if (e.dataTransfer.types.includes("Files")) {
                e.preventDefault();
                setDragging(true);
              }
            }}
            onDragLeave={() => setDragging(false)}
            onDrop={(e) => {
              if (e.dataTransfer.files.length > 0) {
                e.preventDefault();
                setDragging(false);
                addFiles([...e.dataTransfer.files]);
              }
            }}
            className={cn("block w-full resize-y bg-transparent p-3 text-sm leading-normal outline-none placeholder:text-fg-muted", dragging && "bg-accent-muted")}
          />
          {open && (
            <ul
              role="listbox"
              style={{ top: popup.top, left: popup.left }}
              className="absolute z-40 w-[280px] max-w-[calc(100%-8px)] overflow-hidden rounded-md border border-line bg-popover text-sm shadow-[var(--shadow-floating-small)]"
            >
              {suggestions.map((s, i) => (
                <li
                  key={s.key}
                  role="option"
                  aria-selected={i === active}
                  onMouseDown={(e) => {
                    e.preventDefault();
                    choose(s);
                  }}
                  onMouseEnter={() => setActive(i)}
                  className={cn("flex cursor-pointer items-center gap-2 border-b border-line-muted px-3 py-1.5 last:border-0", i === active && "bg-accent-emphasis text-fg-on-emphasis [&_*]:text-inherit")}
                >
                  {s.node}
                </li>
              ))}
            </ul>
          )}
          <div className="flex flex-wrap items-center gap-x-3 gap-y-1 border-t border-dashed border-line px-3 py-1.5 text-xs text-fg-muted">
            <span className="flex items-center gap-1">
              <MarkdownIcon size={16} />
              Markdown is supported
            </span>
            <button type="button" onClick={() => fileInput.current?.click()} className="flex cursor-pointer items-center gap-1 hover:text-fg-accent">
              <PaperclipIcon size={16} />
              {uploading > 0 ? `Uploading ${uploading} file${uploading === 1 ? "" : "s"}…` : "Paste, drop, or click to add files"}
            </button>
            <input
              ref={fileInput}
              type="file"
              multiple
              hidden
              onChange={(e) => {
                addFiles([...(e.target.files ?? [])]);
                e.target.value = "";
              }}
            />
            {uploadError && (
              <span role="alert" className="basis-full text-fg-danger">
                {uploadError}
              </span>
            )}
          </div>
        </div>
      ) : (
        <div style={{ minHeight: minHeight + 30 }} className="p-3">
          {!value.trim() ? (
            <p className="text-sm text-fg-muted">Nothing to preview</p>
          ) : preview.error ? (
            <p className="text-sm text-fg-danger">The preview could not be loaded.</p>
          ) : preview.data ? (
            <MarkdownBody html={preview.data.html} />
          ) : (
            <p className="text-sm text-fg-muted">Loading preview…</p>
          )}
        </div>
      )}
    </div>
  );
}
