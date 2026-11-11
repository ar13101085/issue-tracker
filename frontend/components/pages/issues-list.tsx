"use client";

import {
  ChevronLeftIcon,
  ChevronRightIcon,
  CommentIcon,
  IssueOpenedIcon,
  IssueTracksIcon,
  MilestoneIcon,
  PinIcon,
  SearchIcon,
  TagIcon,
  TriangleDownIcon,
  XCircleFillIcon,
} from "@primer/octicons-react";
import { useSearchParams } from "next/navigation";
import { type ButtonHTMLAttributes, type FormEvent, type ReactNode, useEffect, useState } from "react";
import useSWR from "swr";
import { AvatarStack, labelItems, milestoneItems, TypeBadge, typeItems, useAssignees, useLabels, useMilestones, userItems } from "@/components/issue/pickers";
import { PageError } from "@/components/pages/repos";
import { assignedQuery, createdQuery, IssuesNav, mentionedQuery, recentQuery } from "@/components/shell/issues-layout";
import { Blankslate } from "@/components/shell/blankslate";
import { ActionChoice, ActionHeading, ActionItem, ActionMenu } from "@/components/ui/action-menu";
import { Button } from "@/components/ui/button";
import { Flash, inputClass } from "@/components/ui/form";
import { IssueLabel } from "@/components/ui/issue-label";
import { Counter, IssueStateIcon, RelativeTime, useTitle } from "@/components/ui/misc";
import { SelectPanel } from "@/components/ui/select-panel";
import { api, type ApiError, asApiError, hasRole, type Issue, type IssueList } from "@/lib/api";
import { DEFAULT_QUERY, parseQuery, searchHref, setToken, SORTS, toggleNo, toggleToken } from "@/lib/query";
import { Link, navigate } from "@/lib/router";
import { useRepo, useSession } from "@/lib/session";
import { cn } from "@/lib/utils";

const filterButton = "flex h-8 cursor-pointer items-center gap-2 rounded-md px-3 text-sm whitespace-nowrap text-fg hover:bg-control-transparent-hover";

// The select panel hands its click handler and ref to the trigger, so they must reach the button.
function FilterTrigger({ children, active, ...rest }: { children: ReactNode; active?: boolean } & ButtonHTMLAttributes<HTMLButtonElement>) {
  return (
    <button type="button" {...rest} className={cn(filterButton, active && "font-semibold")}>
      {children}
      <TriangleDownIcon size={16} />
    </button>
  );
}

export function IssuesListPage({ owner, repo }: { owner: string; repo: string }) {
  useTitle(`Issues · ${owner}/${repo}`);
  const full = `${owner}/${repo}`;
  const base = `/${owner}/${repo}/issues`;
  const search = useSearchParams();
  const q = search.get("q") ?? DEFAULT_QUERY;
  const page = Math.max(1, Number(search.get("page")) || 1);
  const parsed = parseQuery(q);
  const { data: repoData } = useRepo(owner, repo);
  const { user } = useSession();
  const canTriage = hasRole(repoData, "triage");

  const listKey = `/repos/${full}/issues?q=${encodeURIComponent(q)}&page=${page}`;
  const { data, error, mutate } = useSWR<IssueList, ApiError>(listKey, api, { keepPreviousData: true });
  const pinned = useSWR<Issue[]>(q === DEFAULT_QUERY && page === 1 ? `/repos/${full}/issues/pinned` : null, api);
  const labels = useLabels(full);
  const milestones = useMilestones(full);
  const assignees = useAssignees(full);

  const [draft, setDraft] = useState(q);
  useEffect(() => setDraft(q), [q]);
  const [picked, setPicked] = useState<number[]>([]);
  useEffect(() => setPicked([]), [listKey]);
  const [bulkError, setBulkError] = useState<string | null>(null);

  const go = (next: string) => navigate(searchHref(base, next));
  function submit(e: FormEvent) {
    e.preventDefault();
    go(draft.trim());
  }

  async function bulk(change: Record<string, unknown>) {
    setBulkError(null);
    try {
      await api(`/repos/${full}/issues/bulk`, { method: "POST", body: { numbers: picked, ...change } });
      setPicked([]);
      await mutate();
    } catch (err) {
      setBulkError(asApiError(err).summary);
    }
  }

  if (error) return <PageError error={error} />;
  const sortLabel = SORTS.find((s) => s.key === parsed.sort)?.label ?? "Newest";
  const pages = data ? Math.max(1, Math.ceil(data.total_count / data.per_page)) : 1;
  const items = data?.items ?? [];
  const allPicked = items.length > 0 && items.every((i) => picked.includes(i.number));
  const none = { key: "__none__", text: "none" };

  return (
    <div className="lg:flex">
      <IssuesNav owner={owner} repo={repo} active={q === assignedQuery ? "assigned" : q === createdQuery ? "created" : q === mentionedQuery ? "mentioned" : q === recentQuery ? "recent" : "issues"} />
      <main className="mx-auto w-full max-w-[1280px] min-w-0 flex-1 p-4 md:p-6">
      {pinned.data && pinned.data.length > 0 && (
        <div className="mb-4 grid gap-3 md:grid-cols-3">
          {pinned.data.map((i) => (
            <Link key={i.number} href={`${base}/${i.number}`} className="flex flex-col gap-1 rounded-md border border-line p-3 hover:bg-canvas-muted">
              <span className="flex items-start gap-2">
                <span className="flex h-5 items-center">
                  <IssueStateIcon issue={i} />
                </span>
                <span className="line-clamp-2 min-w-0 flex-1 text-sm font-semibold">{i.title}</span>
                <PinIcon size={16} className="mt-0.5 shrink-0 text-fg-muted" />
              </span>
              <span className="pl-6 text-xs text-fg-muted">
                #{i.number} opened <RelativeTime iso={i.created_at} /> by {i.author.login}
              </span>
            </Link>
          ))}
        </div>
      )}

      <div className="flex items-center justify-between gap-2">
        <h1 className="text-xl font-semibold">Issues</h1>
        <div className="flex gap-2">
          <Button href={`/${owner}/${repo}/labels`} leading={<TagIcon size={16} />} className="lg:hidden">
            Labels
          </Button>
          <Button href={`/${owner}/${repo}/milestones`} leading={<MilestoneIcon size={16} />} className="lg:hidden">
            Milestones
          </Button>
          <Button variant="primary" href={`${base}/new`}>
            New issue
          </Button>
        </div>
      </div>
      <form onSubmit={submit} role="search" className="relative mt-4 flex min-w-0">
        <input aria-label="Search Issues" value={draft} onChange={(e) => setDraft(e.target.value)} spellCheck={false} className={cn(inputClass, "rounded-r-none pr-8")} />
        {q !== DEFAULT_QUERY && (
          <button type="button" aria-label="Clear filter" onClick={() => go(DEFAULT_QUERY)} className="absolute top-0 right-8 flex h-8 w-8 cursor-pointer items-center justify-center text-fg-muted hover:text-fg">
            <XCircleFillIcon size={16} />
          </button>
        )}
        <button
          type="submit"
          aria-label="Search"
          className="flex h-8 w-8 shrink-0 cursor-pointer items-center justify-center rounded-r-md border border-l-0 border-[var(--control-borderColor-rest)] bg-control text-fg-muted hover:bg-control-hover"
        >
          <SearchIcon size={16} />
        </button>
      </form>
      {user && q !== DEFAULT_QUERY && (
        <p className="mt-2 text-right text-xs">
          <Link href={`${base}/views/new?q=${encodeURIComponent(q)}`} className="text-fg-accent hover:underline">
            Save view
          </Link>
        </p>
      )}

      {bulkError && (
        <Flash className="mt-4" onDismiss={() => setBulkError(null)}>
          {bulkError}
        </Flash>
      )}

      <div className="mt-4 rounded-md border border-line-muted">
        <div className="flex min-h-12 flex-wrap items-center gap-y-1 rounded-t-md bg-canvas-muted p-2">
          {canTriage && (
            <input
              type="checkbox"
              aria-label="Select all issues"
              checked={allPicked}
              onChange={() => setPicked(allPicked ? [] : items.map((i) => i.number))}
              className="mr-2 ml-2 h-4 w-4 cursor-pointer"
            />
          )}
          {picked.length > 0 ? (
            <>
              <span className="px-2 text-sm font-semibold">{picked.length} selected</span>
              <div className="ml-auto flex flex-wrap items-center">
                <ActionMenu align="end" width={220} trigger={<button type="button" className={filterButton}>Mark as <TriangleDownIcon size={16} /></button>}>
                  <ActionItem onSelect={() => bulk({ state: "open" })}>Open</ActionItem>
                  <ActionItem onSelect={() => bulk({ state: "closed", state_reason: "completed" })}>Completed</ActionItem>
                  <ActionItem onSelect={() => bulk({ state: "closed", state_reason: "not_planned" })}>Not planned</ActionItem>
                </ActionMenu>
                <SelectPanel title="Apply labels" placeholder="Filter labels" multi trigger={<FilterTrigger>Label</FilterTrigger>} items={labelItems(labels.data)} selected={[]} onApply={(keys) => bulk({ add_labels: keys })} />
                <SelectPanel title="Assign people" placeholder="Filter users" multi trigger={<FilterTrigger>Assign</FilterTrigger>} items={userItems(assignees.data)} selected={[]} onApply={(keys) => bulk({ add_assignees: keys })} />
                <SelectPanel
                  title="Set milestone"
                  placeholder="Filter milestones"
                  trigger={<FilterTrigger>Milestone</FilterTrigger>}
                  items={[{ ...none, node: <span className="font-semibold">No milestone</span> }, ...milestoneItems(milestones.data, true)]}
                  selected={[]}
                  onApply={([key]) => {
                    const m = milestones.data?.find((x) => x.title === key);
                    if (key) bulk({ milestone: m ? m.number : null });
                  }}
                />
              </div>
            </>
          ) : (
            <>
              <StateTab href={searchHref(base, setToken(q, "state", "open"))} active={parsed.state === "open"}>
                Open {data && <Counter>{data.open_count.toLocaleString("en-US")}</Counter>}
              </StateTab>
              <StateTab href={searchHref(base, setToken(q, "state", "closed"))} active={parsed.state === "closed"}>
                Closed {data && <Counter>{data.closed_count.toLocaleString("en-US")}</Counter>}
              </StateTab>
              <div className="ml-auto flex max-w-full items-center overflow-x-auto [scrollbar-width:none]">
                <SelectPanel
                  title="Filter by author"
                  placeholder="Filter users"
                  trigger={<FilterTrigger active={!!parsed.author}>Author</FilterTrigger>}
                  items={userItems(assignees.data)}
                  selected={parsed.author ? [parsed.author] : []}
                  onApply={([key]) => go(setToken(q, "author", key ?? null))}
                />
                <SelectPanel
                  title="Filter by label"
                  placeholder="Filter labels"
                  loading={!labels.data}
                  trigger={<FilterTrigger active={parsed.labels.length > 0 || parsed.no.includes("label")}>Labels</FilterTrigger>}
                  items={[{ ...none, node: <span className="font-semibold">No labels</span> }, ...labelItems(labels.data)]}
                  selected={parsed.no.includes("label") ? [none.key] : parsed.labels}
                  onToggle={(key) => go(key === none.key ? toggleNo(q, "label") : toggleToken(q, "label", key))}
                />
                <SelectPanel
                  title="Filter by milestone"
                  placeholder="Filter milestones"
                  trigger={<FilterTrigger active={!!parsed.milestone || parsed.no.includes("milestone")}>Milestones</FilterTrigger>}
                  items={[{ ...none, node: <span className="font-semibold">No milestone</span> }, ...milestoneItems(milestones.data)]}
                  selected={parsed.no.includes("milestone") ? [none.key] : parsed.milestone ? [parsed.milestone] : []}
                  onApply={([key]) => go(key === none.key ? toggleNo(q, "milestone") : parsed.no.includes("milestone") && !key ? toggleNo(q, "milestone") : setToken(q, "milestone", key ?? null))}
                />
                <SelectPanel
                  title="Filter by assignee"
                  placeholder="Filter users"
                  trigger={<FilterTrigger active={!!parsed.assignee || parsed.no.includes("assignee")}>Assignees</FilterTrigger>}
                  items={[{ ...none, node: <span className="font-semibold">No assignee</span> }, ...userItems(assignees.data)]}
                  selected={parsed.no.includes("assignee") ? [none.key] : parsed.assignee ? [parsed.assignee] : []}
                  onApply={([key]) => go(key === none.key ? toggleNo(q, "assignee") : parsed.no.includes("assignee") && !key ? toggleNo(q, "assignee") : setToken(q, "assignee", key ?? null))}
                />
                <SelectPanel
                  title="Filter by type"
                  placeholder="Filter types"
                  trigger={<FilterTrigger active={!!parsed.type || parsed.no.includes("type")}>Types</FilterTrigger>}
                  items={[{ ...none, node: <span className="font-semibold">No type</span> }, ...typeItems()]}
                  selected={parsed.no.includes("type") ? [none.key] : parsed.type ? [parsed.type] : []}
                  onApply={([key]) => go(key === none.key ? toggleNo(q, "type") : parsed.no.includes("type") && !key ? toggleNo(q, "type") : setToken(q, "type", key ?? null))}
                />
                <ActionMenu width={240} trigger={<button type="button" className={filterButton}>{sortLabel}<TriangleDownIcon size={16} /></button>}>
                  <ActionHeading>Sort by</ActionHeading>
                  {SORTS.map((s) => (
                    <ActionChoice key={s.key} selected={s.key === parsed.sort} onSelect={() => go(setToken(q, "sort", s.key === "created-desc" ? null : s.key))}>
                      {s.label}
                    </ActionChoice>
                  ))}
                </ActionMenu>
              </div>
            </>
          )}
        </div>

        {data && items.length === 0 && (
          <div className="border-t border-line-muted">
            <Blankslate icon={<IssueOpenedIcon size={24} />} title="No results" description="Try adjusting your search filters." />
          </div>
        )}
        <ul>
          {items.map((issue) => (
            <IssueRow
              key={issue.number}
              issue={issue}
              base={base}
              full={full}
              q={q}
              selectable={canTriage}
              selected={picked.includes(issue.number)}
              onSelect={() => setPicked((p) => (p.includes(issue.number) ? p.filter((n) => n !== issue.number) : [...p, issue.number]))}
            />
          ))}
        </ul>
      </div>

      {pages > 1 && <Pagination page={page} pages={pages} href={(p) => searchHref(base, q, p)} />}
      </main>
    </div>
  );
}

function StateTab({ href, active, children }: { href: string; active: boolean; children: ReactNode }) {
  return (
    <Link
      href={href}
      aria-current={active ? "true" : undefined}
      className={cn("flex h-[30px] items-center gap-2 rounded-md px-2 text-sm whitespace-nowrap hover:bg-control-transparent-hover", active ? "font-semibold text-fg" : "text-fg-muted")}
    >
      {children}
    </Link>
  );
}

// Row geometry follows GitHub's list: a 32px status column with the icon 12px in,
// the title 8px from the top, the 12px meta line under it, and a right-hand column.
function IssueRow({ issue, base, full, q, selectable, selected, onSelect }: { issue: Issue; base: string; full: string; q: string; selectable: boolean; selected: boolean; onSelect: () => void }) {
  const href = `${base}/${issue.number}`;
  const done = issue.sub_issues.filter((s) => s.state === "closed").length;
  const user = (login: string) => (
    <Link href={searchHref(base, setToken(q, "author", login))} className="hover:text-fg-accent hover:underline">
      {login}
    </Link>
  );
  return (
    <li className={cn("flex border-t border-line-muted last:rounded-b-md hover:bg-canvas-muted", selected && "bg-accent-muted hover:bg-accent-muted")}>
      {selectable && (
        <span className="flex h-10 items-center pl-4">
          <input type="checkbox" aria-label={`Select issue ${issue.number}`} checked={selected} onChange={onSelect} className="h-4 w-4 cursor-pointer" />
        </span>
      )}
      <span className="flex h-10 w-9 shrink-0 items-center justify-end pr-1">
        <IssueStateIcon issue={issue} />
      </span>
      <div className="min-w-0 flex-1 pt-2 pb-2.5 pl-1">
        <div className="flex flex-wrap items-center gap-x-1.5 gap-y-1">
          <Link href={href} className="text-base leading-6 font-medium break-words text-fg hover:text-fg-accent hover:underline">
            {issue.title}
          </Link>
          {issue.labels.map((l) => (
            <IssueLabel key={l.name} label={l} href={searchHref(base, toggleToken(setToken(q, "state", "open"), "label", l.name))} />
          ))}
        </div>
        <div className="mt-1 flex flex-wrap items-center gap-x-3 gap-y-1 text-xs text-fg-muted">
          <span className="flex flex-wrap items-center gap-x-1">
            {issue.issue_type && <TypeBadge type={issue.issue_type} className="mr-1" />}
            <span>
              {full}#{issue.number}
            </span>
            <span>·</span>
            {issue.state === "open" ? (
              <span>
                {user(issue.author.login)} opened <RelativeTime iso={issue.created_at} />
              </span>
            ) : (
              <span>
                {user(issue.author.login)} closed <RelativeTime iso={issue.closed_at ?? issue.updated_at} />
              </span>
            )}
          </span>
          {issue.milestone && (
            <Link href={searchHref(base, setToken(q, "milestone", issue.milestone.title))} className="flex items-center gap-1 hover:text-fg-accent">
              <MilestoneIcon size={12} />
              {issue.milestone.title}
            </Link>
          )}
          {issue.sub_issues.length > 0 && (
            <span className="flex items-center gap-1" title="Sub-issues completed">
              <IssueTracksIcon size={12} />
              {done} of {issue.sub_issues.length}
            </span>
          )}
        </div>
      </div>
      <div className="flex h-10 shrink-0 items-center gap-3 pr-4 pl-2">
        {issue.assignees.length > 0 && <AvatarStack logins={issue.assignees.map((a) => a.login)} />}
        {issue.comments > 0 && (
          <Link href={href} aria-label={`${issue.comments} comments`} className="flex items-center gap-1 text-xs text-fg-muted hover:text-fg-accent">
            <CommentIcon size={16} />
            {issue.comments}
          </Link>
        )}
      </div>
    </li>
  );
}

/** Page numbers to show: first, last, and a window around the current one. */
export function pageWindow(page: number, pages: number): (number | "gap")[] {
  const wanted = new Set([1, 2, pages - 1, pages, page - 2, page - 1, page, page + 1, page + 2]);
  const list = [...wanted].filter((p) => p >= 1 && p <= pages).sort((a, b) => a - b);
  const out: (number | "gap")[] = [];
  list.forEach((p, i) => {
    if (i > 0 && p - list[i - 1] > 1) out.push("gap");
    out.push(p);
  });
  return out;
}

export function Pagination({ page, pages, href }: { page: number; pages: number; href: (p: number) => string }) {
  const item = "flex h-8 min-w-8 items-center justify-center gap-1 rounded-md px-2 text-sm";
  const link = cn(item, "text-fg hover:bg-control-transparent-hover");
  const off = cn(item, "text-[var(--fgColor-disabled)]");
  return (
    <nav aria-label="Pagination" className="mt-4 flex flex-wrap items-center justify-center gap-1">
      {page > 1 ? (
        <Link href={href(page - 1)} rel="prev" className={cn(link, "text-fg-accent")}>
          <ChevronLeftIcon size={16} />
          Previous
        </Link>
      ) : (
        <span className={off} aria-disabled="true">
          <ChevronLeftIcon size={16} />
          Previous
        </span>
      )}
      <span className="hidden items-center gap-1 sm:flex">
        {pageWindow(page, pages).map((p, i) =>
          p === "gap" ? (
            <span key={`gap-${i}`} className={off}>
              …
            </span>
          ) : p === page ? (
            <span key={p} aria-current="page" className={cn(item, "bg-accent-emphasis text-fg-on-emphasis")}>
              {p}
            </span>
          ) : (
            <Link key={p} href={href(p)} aria-label={`Page ${p}`} className={link}>
              {p}
            </Link>
          ),
        )}
      </span>
      {page < pages ? (
        <Link href={href(page + 1)} rel="next" className={cn(link, "text-fg-accent")}>
          Next
          <ChevronRightIcon size={16} />
        </Link>
      ) : (
        <span className={off} aria-disabled="true">
          Next
          <ChevronRightIcon size={16} />
        </span>
      )}
    </nav>
  );
}
