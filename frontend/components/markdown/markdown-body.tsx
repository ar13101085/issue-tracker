"use client";

import { type MouseEvent, useEffect, useRef } from "react";
import { navigate } from "@/lib/router";
import { cn } from "@/lib/utils";

const TASK = /^(\s*(?:>\s*)*(?:[-*+]|\d+[.)])\s+)\[([ xX])\](?=\s|$)/;

/** Checks or unchecks the `index`-th task list item (`- [ ]`) in markdown source, skipping code fences. */
export function toggleTask(source: string, index: number, checked: boolean): string {
  let seen = 0;
  let fence: string | null = null;
  return source
    .split("\n")
    .map((line) => {
      const mark = /^\s*(```+|~~~+)/.exec(line)?.[1];
      if (mark) {
        if (!fence) fence = mark[0];
        else if (mark[0] === fence) fence = null;
        return line;
      }
      if (fence || !TASK.test(line)) return line;
      return seen++ === index ? line.replace(TASK, `$1[${checked ? "x" : " "}]`) : line;
    })
    .join("\n");
}

/**
 * Server-rendered, sanitized markdown. Highlights code and keeps in-app links in the SPA.
 * With `onToggleTask` the task list checkboxes can be clicked; it gets the checkbox's position in the text.
 */
export function MarkdownBody({ html, className, onToggleTask }: { html: string; className?: string; onToggleTask?: (index: number, checked: boolean) => void }) {
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const blocks = ref.current?.querySelectorAll<HTMLElement>('pre code[class*="language-"]');
    if (!blocks?.length) return;
    let cancelled = false;
    import("highlight.js/lib/common").then(({ default: hljs }) => {
      if (cancelled) return;
      blocks.forEach((block) => {
        const lang = [...block.classList].find((c) => c.startsWith("language-"))?.slice(9);
        if (!lang || !hljs.getLanguage(lang) || block.dataset.highlighted) return;
        // textContent in, escaped markup out: nothing from the comment is treated as HTML.
        block.innerHTML = hljs.highlight(block.textContent ?? "", { language: lang }).value;
        block.dataset.highlighted = "yes";
      });
    });
    return () => {
      cancelled = true;
    };
  }, [html]);

  // The server renders every checkbox disabled; they are enabled here only for someone who may edit.
  const canToggle = !!onToggleTask;
  useEffect(() => {
    ref.current?.querySelectorAll<HTMLInputElement>('input[type="checkbox"]').forEach((box) => {
      box.disabled = !canToggle;
    });
  }, [html, canToggle]);

  function onClick(e: MouseEvent<HTMLDivElement>) {
    const box = e.target as HTMLElement;
    if (box instanceof HTMLInputElement && box.type === "checkbox" && onToggleTask && ref.current) {
      const all = [...ref.current.querySelectorAll<HTMLInputElement>('input[type="checkbox"]')];
      onToggleTask(all.indexOf(box), box.checked);
      return;
    }
    const link = (e.target as HTMLElement).closest("a");
    const href = link?.getAttribute("href");
    if (!href || e.button !== 0 || e.metaKey || e.ctrlKey || e.shiftKey || e.altKey) return;
    if (href.startsWith("/") && !href.startsWith("//")) {
      e.preventDefault();
      navigate(href);
    }
  }

  if (!html.trim()) return <p className={cn("text-sm text-fg-muted italic", className)}>No description provided.</p>;
  return <div ref={ref} onClick={onClick} className={cn("markdown-body", className)} dangerouslySetInnerHTML={{ __html: html }} />;
}
