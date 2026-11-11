"use client";

import { type MouseEvent, useEffect, useRef } from "react";
import { navigate } from "@/lib/router";
import { cn } from "@/lib/utils";

/** Server-rendered, sanitized markdown. Highlights code and keeps in-app links in the SPA. */
export function MarkdownBody({ html, className }: { html: string; className?: string }) {
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

  function onClick(e: MouseEvent<HTMLDivElement>) {
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
