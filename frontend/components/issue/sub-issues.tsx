"use client";

import { IssueTracksIcon, XIcon } from "@primer/octicons-react";
import useSWR from "swr";
import { IssueStateIcon } from "@/components/ui/misc";
import { SelectPanel } from "@/components/ui/select-panel";
import { api, type Issue, type IssueList } from "@/lib/api";
import { Link } from "@/lib/router";
import { buttonClass } from "@/components/ui/button";

/** The sub-issue list under the issue body, with its progress. */
export function SubIssues({ repo, issue, onAdd, onRemove }: { repo: string; issue: Issue; onAdd: (number: number) => void; onRemove: (number: number) => void }) {
  const canEdit = issue.viewer_can_triage;
  const candidates = useSWR<IssueList>(canEdit ? `/repos/${repo}/issues?q=${encodeURIComponent("is:issue state:open")}&per_page=100` : null, api);
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
