"use client";

import { BellIcon, CheckIcon, DotFillIcon, InboxIcon, UndoIcon } from "@primer/octicons-react";
import { useSearchParams } from "next/navigation";
import useSWR from "swr";
import { Pagination } from "@/components/pages/issues-list";
import { Container, PageError } from "@/components/pages/repos";
import { Blankslate } from "@/components/shell/blankslate";
import { Button } from "@/components/ui/button";
import { IssueStateIcon, RelativeTime, useTitle } from "@/components/ui/misc";
import { api, type ApiError, type Notification, type NotificationList } from "@/lib/api";
import { Link, Redirect } from "@/lib/router";
import { useSession } from "@/lib/session";
import { cn } from "@/lib/utils";

const REASONS: Record<string, string> = {
  mention: "mention",
  assign: "assign",
  author: "author",
  comment: "comment",
  subscribed: "subscribed",
  state_change: "state change",
};

const FILTERS = [
  { key: "inbox", label: "Inbox", icon: <InboxIcon size={16} /> },
  { key: "unread", label: "Unread", icon: <DotFillIcon size={16} /> },
  { key: "done", label: "Done", icon: <CheckIcon size={16} /> },
];

export function NotificationsPage() {
  useTitle("Notifications · Issue Tracker");
  const { user } = useSession();
  const search = useSearchParams();
  const filter = FILTERS.some((f) => f.key === search.get("filter")) ? search.get("filter")! : "inbox";
  const page = Math.max(1, Number(search.get("page")) || 1);
  const { data, error, mutate } = useSWR<NotificationList, ApiError>(user ? `/notifications?filter=${filter}&page=${page}` : null, api, { keepPreviousData: true });

  if (user === undefined) return null;
  if (user === null) return <Redirect to="/login?return_to=/notifications" />;
  if (error) return <PageError error={error} />;

  const change = async (n: Notification, body: Record<string, boolean>) => {
    await api(`/notifications/${n.id}`, { method: "PATCH", body });
    await mutate();
  };
  const readAll = async () => {
    await api("/notifications/read", { method: "POST" });
    await mutate();
  };
  const href = (f: string, p = 1) => `/notifications${f === "inbox" && p === 1 ? "" : `?filter=${f}${p > 1 ? `&page=${p}` : ""}`}`;
  const pages = data ? Math.max(1, Math.ceil(data.total_count / 25)) : 1;
  const rowButton = "flex h-7 w-7 cursor-pointer items-center justify-center rounded-md text-fg-muted hover:bg-control-transparent-hover hover:text-fg";

  return (
    <Container>
      <div className="flex flex-col gap-6 md:flex-row">
        <nav aria-label="Filters" className="flex shrink-0 gap-1 overflow-x-auto md:w-[220px] md:flex-col">
          {FILTERS.map((f) => (
            <Link
              key={f.key}
              href={href(f.key)}
              aria-current={filter === f.key ? "page" : undefined}
              className={cn("flex h-8 items-center gap-2 rounded-md px-2 text-sm whitespace-nowrap hover:bg-control-transparent-hover", filter === f.key && "bg-control-transparent-hover font-semibold")}
            >
              <span className="text-fg-muted">{f.icon}</span>
              <span className="flex-1">{f.label}</span>
              {f.key === "inbox" && !!data?.unread_count && <span className="rounded-full bg-accent-emphasis px-1.5 text-xs font-medium text-fg-on-emphasis">{data.unread_count}</span>}
            </Link>
          ))}
        </nav>

        <div className="min-w-0 flex-1">
          <div className="rounded-md border border-line">
            <div className="flex min-h-12 items-center justify-between gap-2 rounded-t-md border-b border-line bg-canvas-muted px-4 py-2">
              <h1 className="text-sm font-semibold">
                {FILTERS.find((f) => f.key === filter)?.label}
                {data ? <span className="ml-1 font-normal text-fg-muted">{data.total_count}</span> : null}
              </h1>
              {filter !== "done" && !!data?.unread_count && (
                <Button size="sm" onClick={readAll}>
                  Mark all as read
                </Button>
              )}
            </div>
            {data && data.items.length === 0 && (
              <Blankslate icon={<BellIcon size={24} />} title={filter === "done" ? "Nothing marked as done" : "All caught up!"} description="Activity on issues you follow shows up here." />
            )}
            <ul className="divide-y divide-line-muted">
              {data?.items.map((n) => (
                <li key={n.id} className={cn("group flex items-start gap-2 px-3 py-2 md:px-4", n.unread ? "bg-canvas" : "bg-canvas-muted")}>
                  <span className="flex h-5 w-2 shrink-0 items-center" title={n.unread ? "Unread" : undefined}>
                    {n.unread && <span className="h-2 w-2 rounded-full bg-accent-emphasis" />}
                  </span>
                  <span className="flex h-5 items-center">
                    <IssueStateIcon issue={n.issue} />
                  </span>
                  <Link href={`/${n.repo}/issues/${n.issue.number}`} className="min-w-0 flex-1">
                    <span className="block text-xs text-fg-muted">
                      {n.repo} <span>#{n.issue.number}</span>
                    </span>
                    <span className={cn("block text-sm break-words", n.unread && "font-semibold")}>{n.issue.title}</span>
                    <span className="block text-xs text-fg-muted">
                      {n.last_actor.login} · {REASONS[n.reason] ?? n.reason} · <RelativeTime iso={n.updated_at} />
                    </span>
                  </Link>
                  <span className="flex shrink-0 items-center">
                    {n.done ? (
                      <button type="button" aria-label="Move back to inbox" title="Move to inbox" className={rowButton} onClick={() => change(n, { done: false })}>
                        <UndoIcon size={16} />
                      </button>
                    ) : (
                      <>
                        <button type="button" aria-label={n.unread ? "Mark as read" : "Mark as unread"} title={n.unread ? "Mark as read" : "Mark as unread"} className={rowButton} onClick={() => change(n, { unread: !n.unread })}>
                          <DotFillIcon size={16} className={n.unread ? "" : "text-fg-accent"} />
                        </button>
                        <button type="button" aria-label="Mark as done" title="Done" className={rowButton} onClick={() => change(n, { done: true })}>
                          <CheckIcon size={16} />
                        </button>
                      </>
                    )}
                  </span>
                </li>
              ))}
            </ul>
          </div>
          {pages > 1 && <Pagination page={page} pages={pages} href={(p) => href(filter, p)} />}
        </div>
      </div>
    </Container>
  );
}
