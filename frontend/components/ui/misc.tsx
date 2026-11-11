"use client";

import { IssueClosedIcon, IssueOpenedIcon, SkipIcon } from "@primer/octicons-react";
import { useEffect, useState } from "react";
import type { Issue } from "@/lib/api";
import { fullTime, relativeTime } from "@/lib/time";
import { cn } from "@/lib/utils";

/** Timestamp that keeps itself current. */
export function RelativeTime({ iso }: { iso: string }) {
  const [, tick] = useState(0);
  useEffect(() => {
    const id = setInterval(() => tick((n) => n + 1), 60_000);
    return () => clearInterval(id);
  }, []);
  return (
    <time dateTime={iso} title={fullTime(iso)}>
      {relativeTime(iso)}
    </time>
  );
}

export function isNotPlanned(issue: Pick<Issue, "state" | "state_reason">) {
  return issue.state === "closed" && (issue.state_reason === "not_planned" || issue.state_reason === "duplicate");
}

/** The colored state icon in front of an issue. */
export function IssueStateIcon({ issue, size = 16 }: { issue: Pick<Issue, "state" | "state_reason">; size?: number }) {
  if (issue.state === "open") return <IssueOpenedIcon size={size} className="text-fg-success" aria-label="Open issue" />;
  if (isNotPlanned(issue)) return <SkipIcon size={size} className="text-fg-muted" aria-label="Closed as not planned issue" />;
  return <IssueClosedIcon size={size} className="text-fg-done" aria-label="Closed issue" />;
}

/** Primer StateLabel pill. */
export function StateLabel({ issue }: { issue: Pick<Issue, "state" | "state_reason"> }) {
  const open = issue.state === "open";
  const skipped = isNotPlanned(issue);
  return (
    <span
      className={cn(
        "inline-flex h-8 items-center gap-1 rounded-full px-3 text-sm font-semibold text-fg-on-emphasis",
        open ? "bg-success-emphasis" : skipped ? "bg-[var(--bgColor-neutral-emphasis)]" : "bg-done-emphasis",
      )}
    >
      {open ? <IssueOpenedIcon size={16} /> : skipped ? <SkipIcon size={16} /> : <IssueClosedIcon size={16} />}
      {open ? "Open" : "Closed"}
    </span>
  );
}

/** Small outlined pill: Public / Private. */
export function Label({ children }: { children: React.ReactNode }) {
  return (
    <span className="inline-flex h-5 items-center rounded-full border border-line px-[7px] text-xs font-medium text-fg-muted">
      {children}
    </span>
  );
}

/** Counter bubble used in tabs. */
export function Counter({ children }: { children: React.ReactNode }) {
  return (
    <span className="inline-flex h-5 min-w-5 items-center justify-center rounded-full bg-neutral-muted px-1.5 text-xs font-semibold">
      {children}
    </span>
  );
}

export function useTitle(title: string) {
  useEffect(() => {
    document.title = title;
  }, [title]);
}
