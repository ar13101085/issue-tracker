"use client";

// Data and list items shared by the list page filters and the issue sidebar.

import useSWR from "swr";
import { Avatar } from "@/components/ui/avatar";
import type { PanelItem } from "@/components/ui/select-panel";
import { api, ISSUE_TYPES, type LabelWithCount, type Milestone } from "@/lib/api";
import { cn } from "@/lib/utils";

export function useLabels(repo: string, enabled = true) {
  return useSWR<LabelWithCount[]>(enabled ? `/repos/${repo}/labels` : null, api);
}

export function useMilestones(repo: string, enabled = true) {
  return useSWR<Milestone[]>(enabled ? `/repos/${repo}/milestones?state=all` : null, api);
}

export function useAssignees(repo: string, enabled = true) {
  return useSWR<{ login: string }[]>(enabled ? `/repos/${repo}/assignees` : null, api);
}

export function labelItems(labels: LabelWithCount[] | undefined): PanelItem[] {
  return (labels ?? []).map((l) => ({
    key: l.name,
    text: `${l.name} ${l.description}`,
    description: l.description || undefined,
    node: (
      <>
        <span className="h-3.5 w-3.5 shrink-0 rounded-full border border-line-muted" style={{ background: `#${l.color}` }} />
        <span className="truncate font-semibold">{l.name}</span>
      </>
    ),
  }));
}

export function userItems(users: { login: string }[] | undefined): PanelItem[] {
  return (users ?? []).map((u) => ({
    key: u.login,
    text: u.login,
    node: (
      <>
        <Avatar login={u.login} size={20} />
        <span className="truncate font-semibold">{u.login}</span>
      </>
    ),
  }));
}

export function milestoneItems(milestones: Milestone[] | undefined, openOnly = false): PanelItem[] {
  return (milestones ?? [])
    .filter((m) => !openOnly || m.state === "open")
    .map((m) => ({
      key: m.title,
      text: m.title,
      description: m.state === "closed" ? "Closed" : m.due_on ? `Due by ${new Date(m.due_on).toLocaleDateString("en-US", { dateStyle: "medium" })}` : "No due date",
      node: <span className="truncate font-semibold">{m.title}</span>,
    }));
}

const TYPE_TONE: Record<string, string> = {
  Bug: "border-[var(--borderColor-danger-muted)] bg-[var(--bgColor-danger-muted)] text-fg-danger",
  Feature: "border-[var(--borderColor-accent-muted)] bg-[var(--bgColor-accent-muted)] text-fg-accent",
  Task: "border-[var(--borderColor-attention-muted)] bg-[var(--bgColor-attention-muted)] text-fg-attention",
};

/** The issue type pill: Bug, Feature or Task. */
export function TypeBadge({ type, className }: { type: string; className?: string }) {
  return (
    <span className={cn("inline-flex h-5 items-center rounded-full border px-[7px] text-xs font-semibold whitespace-nowrap", TYPE_TONE[type] ?? "border-line text-fg-muted", className)}>
      {type}
    </span>
  );
}

export function typeItems(): PanelItem[] {
  return ISSUE_TYPES.map((t) => ({ key: t, text: t, node: <TypeBadge type={t} /> }));
}

/** Overlapping avatars, newest on top, as on the right of an issue row. */
export function AvatarStack({ logins, size = 20 }: { logins: string[]; size?: number }) {
  return (
    <span className="flex flex-row-reverse" title={`Assigned to ${logins.join(", ")}`}>
      {[...logins].reverse().map((login, i) => (
        <Avatar key={login} login={login} size={size} className={cn("ring-2 ring-canvas", i < logins.length - 1 && "-ml-2")} />
      ))}
    </span>
  );
}
