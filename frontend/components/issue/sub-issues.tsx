"use client";

import { IssueTracksIcon, XIcon } from "@primer/octicons-react";
import type { ButtonHTMLAttributes, ReactNode } from "react";
import useSWR from "swr";
import { IssueStateIcon } from "@/components/ui/misc";
import { SelectPanel } from "@/components/ui/select-panel";
import { api, type Issue, type IssueList } from "@/lib/api";
import { Link } from "@/lib/router";
import { buttonClass } from "@/components/ui/button";
import { SidebarSection } from "@/components/issue/sidebar";

const candidatesKey = (repo: string) => `/repos/${repo}/issues?q=${encodeURIComponent("is:issue state:open")}&per_page=100`;

type IssueLike = { number: number; title: string; state: Issue["state"]; state_reason: Issue["state_reason"] };

function panelItems(issues: IssueLike[]) {
  return issues.map((i) => ({
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
}

// The select panel hands its click handler and ref to the trigger, so they must reach the button.
function RelationButton({ children, ...rest }: { children: ReactNode } & ButtonHTMLAttributes<HTMLButtonElement>) {
  return (
    <button type="button" {...rest} className="cursor-pointer text-xs text-fg-muted hover:text-fg-accent hover:underline">
      {children}
    </button>
  );
}

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
 * Sidebar section with the parent issue and what blocks, or is blocked by, this issue.
 * Triage and up can change each of them.
 */
export function RelationshipsSection({
  repo,
  issue,
  onSetParent,
  onBlockedBy,
  onBlocking,
}: {
  repo: string;
  issue: Issue;
  onSetParent: (number: number | null) => void;
  /** `on` says whether the link is being added or removed. */
  onBlockedBy: (number: number, on: boolean) => void;
  onBlocking: (number: number, on: boolean) => void;
}) {
  const canEdit = issue.viewer_can_triage;
  const candidates = useSWR<IssueList>(canEdit ? candidatesKey(repo) : null, api);
  const open: IssueLike[] = (candidates.data?.items ?? []).filter((i) => i.number !== issue.number);
  // Linked issues that are closed are not in the open list, but must stay selectable so they can be removed.
  const withLinked = (linked: IssueLike[], hidden: number[]) => {
    const skip = new Set(hidden);
    const rest = open.filter((i) => !skip.has(i.number) && !linked.some((l) => l.number === i.number));
    return panelItems([...linked, ...rest]);
  };
  const subs = issue.sub_issues.map((s) => s.number);
  const blockedBy = issue.blocked_by.map((i) => i.number);
  const blocking = issue.blocking.map((i) => i.number);
  const empty = !issue.parent && blockedBy.length === 0 && blocking.length === 0;
  const common = { placeholder: "Search issues", loading: !candidates.data, empty: "No open issues to choose", align: "end" as const };

  return (
    <SidebarSection title="Relationships">
      <div className="flex flex-col gap-3">
        {issue.parent && <RelationList repo={repo} label="Parent issue" issues={[issue.parent]} />}
        <RelationList repo={repo} label="Blocked by" issues={issue.blocked_by} />
        <RelationList repo={repo} label="Blocking" issues={issue.blocking} />
        {empty && <p className="text-fg-muted">None yet</p>}
        {canEdit && (
          <div className="flex flex-col items-start gap-1">
            <SelectPanel
              {...common}
              title={issue.parent ? "Change or remove parent issue" : "Add parent issue"}
              trigger={<RelationButton>{issue.parent ? "Change or remove parent" : "Add parent"}</RelationButton>}
              items={withLinked(issue.parent ? [issue.parent] : [], subs)}
              selected={issue.parent ? [String(issue.parent.number)] : []}
              onApply={([key]) => onSetParent(key ? Number(key) : null)}
            />
            <SelectPanel
              {...common}
              title="Issues that block this one"
              trigger={<RelationButton>{blockedBy.length ? "Add or change blocked by" : "Mark as blocked by"}</RelationButton>}
              items={withLinked(issue.blocked_by, blocking)}
              selected={blockedBy.map(String)}
              onToggle={(key) => onBlockedBy(Number(key), !blockedBy.includes(Number(key)))}
            />
            <SelectPanel
              {...common}
              title="Issues this one blocks"
              trigger={<RelationButton>{blocking.length ? "Add or change blocking" : "Mark as blocking"}</RelationButton>}
              items={withLinked(issue.blocking, blockedBy)}
              selected={blocking.map(String)}
              onToggle={(key) => onBlocking(Number(key), !blocking.includes(Number(key)))}
            />
          </div>
        )}
      </div>
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
        <div className="mt-2">
          <SelectPanel
            align="start"
            title="Add existing issue as sub-issue"
            placeholder="Search issues"
            loading={!candidates.data}
            empty="No open issues to add"
            trigger={
              <button type="button" className={buttonClass("default", "sm")}>
                <IssueTracksIcon size={16} />
                Add sub-issue
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
