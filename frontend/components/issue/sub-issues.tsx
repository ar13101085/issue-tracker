"use client";

import { ArrowLeftIcon, CheckIcon, IssueTracksIcon, SearchIcon, XIcon } from "@primer/octicons-react";
import { Popover } from "@base-ui/react/popover";
import { useState } from "react";
import useSWR from "swr";
import { IssueStateIcon } from "@/components/ui/misc";
import { SelectPanel } from "@/components/ui/select-panel";
import { api, type Issue, type IssueList } from "@/lib/api";
import { Link } from "@/lib/router";
import { buttonClass } from "@/components/ui/button";
import { Gear, SidebarSection } from "@/components/issue/sidebar";
import { cn } from "@/lib/utils";

const candidatesKey = (repo: string) => `/repos/${repo}/issues?q=${encodeURIComponent("is:issue state:open")}&per_page=100`;

type IssueLike = { number: number; title: string; state: Issue["state"]; state_reason: Issue["state_reason"] };
export type RelationKind = "parent" | "blocked_by" | "blocking" | "related";

const RELATIONS: { kind: RelationKind; add: string; change: string; label: string; hint: string }[] = [
  { kind: "parent", add: "Add parent", change: "Change or remove parent", label: "Parent issue", hint: "Select the parent of the current issue" },
  { kind: "blocked_by", add: "Mark as blocked by", change: "Add or change blocked by", label: "Blocked by", hint: "Mark issues that block the current issue" },
  { kind: "blocking", add: "Mark as blocking", change: "Add or change blocking", label: "Blocking", hint: "Mark issues that the current issue blocks" },
  { kind: "related", add: "Add relates to", change: "Add or change relates to", label: "Relates to", hint: "Mark issues that relate to the current issue" },
];

function RelationList({ repo, label, issues }: { repo: string; label: string; issues: IssueLike[] }) {
  if (issues.length === 0) return null;
  return (
    <div>
      <p className="text-xs text-fg-muted">{label}</p>
      <ul className="mt-1 flex flex-col gap-1">
        {issues.map((i) => (
          <li key={i.number}>
            <Link href={`/${repo}/issues/${i.number}`} className="flex items-start gap-2 text-sm hover:text-fg-accent">
              <span className="flex h-5 items-center">
                <IssueStateIcon issue={i} />
              </span>
              <span className="min-w-0">
                <span className="font-semibold break-words">{i.title}</span> <span className="text-fg-muted">#{i.number}</span>
              </span>
            </Link>
          </li>
        ))}
      </ul>
    </div>
  );
}

/**
 * Sidebar section with the parent issue and the issues this one is blocked by, blocks or relates to.
 * The gear opens a menu of relationship kinds; choosing one swaps the menu for an issue picker.
 * Triage and up can change them. `onChange` gets the issues to link and to unlink.
 */
export function RelationshipsSection({ repo, issue, onChange }: { repo: string; issue: Issue; onChange: (kind: RelationKind, add: number[], remove: number[]) => void }) {
  const canEdit = issue.viewer_can_triage;
  const [open, setOpen] = useState(false);
  const [kind, setKind] = useState<RelationKind | null>(null);
  const [draft, setDraft] = useState<number[]>([]);
  const [filter, setFilter] = useState("");
  const candidates = useSWR<IssueList>(canEdit && open ? candidatesKey(repo) : null, api);

  const linked: Record<RelationKind, IssueLike[]> = {
    parent: issue.parent ? [issue.parent] : [],
    blocked_by: issue.blocked_by,
    blocking: issue.blocking,
    related: issue.related,
  };
  // Links that cannot be combined: a sub-issue as parent, or blocking in both directions.
  const hidden: Record<RelationKind, IssueLike[]> = { parent: issue.sub_issues, blocked_by: issue.blocking, blocking: issue.blocked_by, related: [] };
  const empty = RELATIONS.every((r) => linked[r.kind].length === 0);

  function choose(next: RelationKind) {
    setKind(next);
    setDraft(linked[next].map((i) => i.number));
    setFilter("");
  }

  function apply() {
    if (!kind) return;
    const before = linked[kind].map((i) => i.number);
    const add = draft.filter((n) => !before.includes(n));
    const remove = before.filter((n) => !draft.includes(n));
    if (add.length || remove.length) onChange(kind, add, remove);
  }

  function close(next: boolean) {
    if (!next) {
      apply();
      setKind(null);
    }
    setOpen(next);
  }

  function pick(number: number) {
    if (kind === "parent") {
      // One parent: choosing applies at once, choosing it again removes it.
      const before = issue.parent?.number;
      onChange("parent", before === number ? [] : [number], before === undefined ? [] : [before]);
      setKind(null);
      setOpen(false);
    } else {
      setDraft((d) => (d.includes(number) ? d.filter((n) => n !== number) : [...d, number]));
    }
  }

  const relation = RELATIONS.find((r) => r.kind === kind);
  const skip = new Set([issue.number, ...(kind ? hidden[kind].map((i) => i.number) : [])]);
  const current = kind ? linked[kind] : [];
  // Linked issues that are closed are not in the open list, but must stay listed so they can be removed.
  const pool: IssueLike[] = [...current, ...(candidates.data?.items ?? []).filter((i) => !current.some((c) => c.number === i.number))].filter((i) => !skip.has(i.number));
  const needle = filter.trim().toLowerCase();
  const shown = needle ? pool.filter((i) => `${i.title} #${i.number}`.toLowerCase().includes(needle)) : pool;
  const menuItem = "flex min-h-[33px] w-full cursor-pointer items-center rounded-md px-2 py-1.5 text-left text-sm hover:bg-control-transparent-hover";

  return (
    <SidebarSection
      title="Relationships"
      action={
        canEdit && (
          <Popover.Root open={open} onOpenChange={close}>
            <Popover.Trigger render={<Gear label="Edit Relationships" />} />
            <Popover.Portal>
              <Popover.Positioner align="end" sideOffset={4} className="z-50">
                <Popover.Popup className="flex max-h-[min(480px,70vh)] w-[320px] max-w-[calc(100vw-16px)] flex-col rounded-xl bg-popover text-sm shadow-[var(--shadow-floating-small)] outline-none">
                  {!relation ? (
                    <ul role="menu" aria-label="Relationships" className="p-2">
                      {RELATIONS.map((r) => (
                        <li key={r.kind} role="none">
                          <button type="button" role="menuitem" className={menuItem} onClick={() => choose(r.kind)}>
                            {linked[r.kind].length ? r.change : r.add}
                          </button>
                        </li>
                      ))}
                    </ul>
                  ) : (
                    <>
                      <header className="flex items-start gap-2 px-2 pt-2">
                        <button type="button" aria-label="Back to relationships" onClick={() => { apply(); setKind(null); }} className="flex h-7 w-7 shrink-0 cursor-pointer items-center justify-center rounded-md text-fg-muted hover:bg-control-transparent-hover">
                          <ArrowLeftIcon size={16} />
                        </button>
                        <div className="min-w-0 py-1">
                          <Popover.Title className="truncate text-sm leading-5 font-semibold">{repo}</Popover.Title>
                          <p className="truncate text-xs text-fg-muted">{relation.hint}</p>
                        </div>
                      </header>
                      <div className="p-2">
                        <label className="flex h-8 items-center gap-2 rounded-md border border-[var(--control-borderColor-rest)] bg-canvas pl-2 focus-within:border-fg-accent">
                          <SearchIcon size={16} className="shrink-0 text-fg-muted" />
                          <input autoFocus value={filter} onChange={(e) => setFilter(e.target.value)} placeholder="Search issues" aria-label="Search issues" className="h-full min-w-0 flex-1 bg-transparent pr-2 outline-none" />
                        </label>
                      </div>
                      <ul role="listbox" aria-multiselectable={kind !== "parent"} className="min-h-0 flex-1 overflow-y-auto px-2 pb-2">
                        {!candidates.data && <li className="px-2 py-4 text-center text-fg-muted">Loading…</li>}
                        {candidates.data && shown.length === 0 && <li className="px-2 py-4 text-center text-fg-muted">No open issues to choose</li>}
                        {shown.map((i) => {
                          const on = kind === "parent" ? issue.parent?.number === i.number : draft.includes(i.number);
                          return (
                            <li key={i.number} role="option" aria-selected={on}>
                              <button type="button" onClick={() => pick(i.number)} className="flex w-full cursor-pointer items-start gap-2 rounded-md px-2 py-1.5 text-left leading-5 hover:bg-control-transparent-hover">
                                <span
                                  className={cn(
                                    "mt-0.5 flex h-4 w-4 shrink-0 items-center justify-center",
                                    kind !== "parent" && "rounded-[3px] border",
                                    kind !== "parent" && (on ? "border-accent-emphasis bg-accent-emphasis text-fg-on-emphasis" : "border-[var(--control-borderColor-emphasis)]"),
                                  )}
                                >
                                  {on && <CheckIcon size={kind === "parent" ? 16 : 12} />}
                                </span>
                                <span className="flex h-5 shrink-0 items-center">
                                  <IssueStateIcon issue={i} />
                                </span>
                                <span className="min-w-0 flex-1 break-words">{i.title}</span>
                                <span className="shrink-0 text-fg-muted">#{i.number}</span>
                              </button>
                            </li>
                          );
                        })}
                      </ul>
                    </>
                  )}
                </Popover.Popup>
              </Popover.Positioner>
            </Popover.Portal>
          </Popover.Root>
        )
      }
    >
      {empty ? (
        <p className="text-fg-muted">None yet</p>
      ) : (
        <div className="flex flex-col gap-3">
          {RELATIONS.map((r) => (
            <RelationList key={r.kind} repo={repo} label={r.label} issues={linked[r.kind]} />
          ))}
        </div>
      )}
    </SidebarSection>
  );
}

/** The sub-issue list under the issue body, with its progress. */
export function SubIssues({ repo, issue, onAdd, onRemove }: { repo: string; issue: Issue; onAdd: (number: number) => void; onRemove: (number: number) => void }) {
  const canEdit = issue.viewer_can_triage;
  const candidates = useSWR<IssueList>(canEdit ? candidatesKey(repo) : null, api);
  if (issue.sub_issues.length === 0 && !canEdit) return null;
  const done = issue.sub_issues.filter((s) => s.state === "closed").length;
  const total = issue.sub_issues.length;
  const taken = new Set([issue.number, ...issue.sub_issues.map((s) => s.number), ...(issue.parent ? [issue.parent.number] : [])]);
  const items = (candidates.data?.items ?? [])
    .filter((i) => !taken.has(i.number) && !i.parent)
    .map((i) => ({
      key: String(i.number),
      text: `${i.title} #${i.number}`,
      node: (
        <>
          <IssueStateIcon issue={i} />
          <span className="min-w-0 flex-1 truncate">{i.title}</span>
          <span className="text-fg-muted">#{i.number}</span>
        </>
      ),
    }));

  return (
    <div className="mt-4">
      {total > 0 && (
        <div className="rounded-md border border-line">
          <div className="flex items-center gap-2 rounded-t-md bg-canvas-muted px-3 py-2 text-sm">
            <IssueTracksIcon size={16} className="text-fg-muted" />
            <span className="font-semibold">Sub-issues</span>
            <span className="text-fg-muted">
              {done} of {total}
            </span>
            <span className="ml-auto h-2 w-24 overflow-hidden rounded-full bg-neutral-muted">
              <span className="block h-full rounded-full bg-done-emphasis" style={{ width: `${Math.round((done / total) * 100)}%` }} />
            </span>
          </div>
          <ul>
            {issue.sub_issues.map((s) => (
              <li key={s.number} className="flex items-center gap-2 border-t border-line-muted px-3 py-2 text-sm">
                <IssueStateIcon issue={s} />
                <Link href={`/${repo}/issues/${s.number}`} className="min-w-0 flex-1 truncate font-semibold hover:text-fg-accent">
                  {s.title} <span className="font-normal text-fg-muted">#{s.number}</span>
                </Link>
                {canEdit && (
                  <button type="button" aria-label={`Remove sub-issue ${s.number}`} onClick={() => onRemove(s.number)} className="flex h-6 w-6 cursor-pointer items-center justify-center rounded-md text-fg-muted hover:bg-control-transparent-hover hover:text-fg-danger">
                    <XIcon size={16} />
                  </button>
                )}
              </li>
            ))}
          </ul>
        </div>
      )}
      {canEdit && (
        <div className="mt-2 flex flex-wrap gap-2">
          <Link href={`/${repo}/issues/new?parent=${issue.number}`} className={buttonClass("default", "sm")}>
            <IssueTracksIcon size={16} />
            Create sub-issue
          </Link>
          <SelectPanel
            align="start"
            title="Add existing issue as sub-issue"
            placeholder="Search issues"
            loading={!candidates.data}
            empty="No open issues to add"
            trigger={
              <button type="button" className={buttonClass("default", "sm")}>
                Add existing issue
              </button>
            }
            items={items}
            selected={[]}
            onToggle={(key) => onAdd(Number(key))}
          />
        </div>
      )}
    </div>
  );
}
