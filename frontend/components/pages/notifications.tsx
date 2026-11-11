"use client";

import { BellIcon, CheckIcon, InboxIcon, ReadIcon, UndoIcon, UnreadIcon } from "@primer/octicons-react";
import { useSearchParams } from "next/navigation";
import useSWR from "swr";
import { Pagination } from "@/components/pages/issues-list";
import { PageError } from "@/components/pages/repos";
import { Blankslate } from "@/components/shell/blankslate";
import { Avatar } from "@/components/ui/avatar";
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

const FILTERS = ["inbox", "unread", "done"];

export function NotificationsPage() {
  useTitle("Notifications · Issue Tracker");
  const { user } = useSession();
  const search = useSearchParams();
  const filter = FILTERS.includes(search.get("filter") ?? "") ? search.get("filter")! : "inbox";
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
  const side = (key: string, label: string, icon: React.ReactNode, active: boolean) => (
    <li>
      <Link
        href={href(key)}
        aria-current={active ? "page" : undefined}
        className={cn(
          "relative flex h-[33px] items-center gap-2 rounded-md px-2 text-sm whitespace-nowrap hover:bg-control-transparent-hover",
          active && "bg-[var(--control-transparent-bgColor-selected)] font-semibold md:before:absolute md:before:top-1 md:before:bottom-1 md:before:-left-2 md:before:w-1 md:before:rounded-md md:before:bg-accent-emphasis",
        )}
      >
        <span className="flex w-4 justify-center text-fg-muted">{icon}</span>
        <span className="flex-1">{label}</span>
        {key === "inbox" && !!data?.unread_count && <span className="inline-flex h-5 items-center rounded-full bg-neutral-muted px-[7px] text-xs font-medium">{data.unread_count}</span>}
      </Link>
    </li>
  );
  const segment = (key: string, label: string) => (
    <li className={cn("flex rounded-md border p-1", filter === key ? "border-[var(--control-borderColor-rest)] bg-canvas" : "border-transparent")}>
      <Link href={href(key)} aria-current={filter === key ? "true" : undefined} className={cn("flex h-[22px] items-center rounded px-2 text-sm", filter === key ? "font-semibold" : "hover:bg-control-transparent-hover")}>
        {label}
      </Link>
    </li>
  );

  return (
    <main className="px-4 pb-4 md:flex">
      <nav aria-label="Filters" className="shrink-0 pt-4 md:w-[206px] md:pb-10">
        <ul className="flex gap-1 md:block">
          {side("inbox", "Inbox", <InboxIcon size={16} />, filter !== "done")}
          {side("done", "Done", <CheckIcon size={16} />, filter === "done")}
        </ul>
      </nav>

      <div className="min-w-0 flex-1 pt-4 md:pl-4">
        {filter !== "done" && (
          <div className="mb-4 flex items-center gap-2">
            <ul className="inline-flex rounded-md bg-[var(--controlTrack-bgColor-rest,var(--bgColor-neutral-muted))]">
              {segment("inbox", "All")}
              {segment("unread", "Unread")}
            </ul>
          </div>
        )}
        <div className="rounded-md border border-line">
          <div className="flex min-h-[52px] items-center justify-between gap-2 rounded-t-md border-b border-line bg-canvas-muted py-2 pr-4 pl-4">
            <h1 className="text-sm font-medium">
              {filter === "done" ? "Done" : filter === "unread" ? "Unread" : "Inbox"}
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
          <ul>
            {data?.items.map((n) => (
              <li key={n.id} className={cn("group flex min-h-14 items-center border-b border-line-muted pl-2 last:rounded-b-md last:border-b-0", n.unread ? "bg-canvas" : "bg-canvas-muted")}>
                <span className="flex w-[37px] shrink-0 items-center self-start pt-[21px]" title={n.unread ? "Unread" : undefined}>
                  {n.unread && <span className="h-2 w-2 rounded-full bg-accent-emphasis" />}
                </span>
                <Link href={`/${n.repo}/issues/${n.issue.number}`} className="flex min-w-0 flex-1 py-2">
                  <span className="mr-2 flex h-[21px] shrink-0 items-center">
                    <IssueStateIcon issue={n.issue} />
                  </span>
                  <span className="min-w-0 flex-1">
                    <span className="block truncate text-xs leading-[18px] text-fg-muted">
                      {n.repo} <span>#{n.issue.number}</span>
                    </span>
                    <span className={cn("block text-sm leading-[21px] break-words", n.unread ? "text-fg" : "text-fg-muted")}>{n.issue.title}</span>
                  </span>
                </Link>
                <span className="hidden w-[92px] shrink-0 truncate pr-4 pl-2 text-xs leading-[18px] sm:block">{REASONS[n.reason] ?? n.reason}</span>
                <span className="hidden shrink-0 sm:block" title={n.last_actor.login}>
                  <Avatar login={n.last_actor.login} size={24} />
                </span>
                <span className="flex w-[100px] shrink-0 items-center justify-end pr-4 md:w-[140px]">
                  <span className="text-xs leading-5 text-fg-muted group-focus-within:hidden group-hover:hidden">
                    <RelativeTime iso={n.updated_at} />
                  </span>
                  <span className="hidden items-center group-focus-within:flex group-hover:flex">
                    {n.done ? (
                      <button type="button" aria-label="Move back to inbox" title="Move to inbox" className={rowButton} onClick={() => change(n, { done: false })}>
                        <UndoIcon size={16} />
                      </button>
                    ) : (
                      <>
                        <button type="button" aria-label={n.unread ? "Mark as read" : "Mark as unread"} title={n.unread ? "Mark as read" : "Mark as unread"} className={rowButton} onClick={() => change(n, { unread: !n.unread })}>
                          {n.unread ? <ReadIcon size={16} /> : <UnreadIcon size={16} />}
                        </button>
                        <button type="button" aria-label="Mark as done" title="Done" className={rowButton} onClick={() => change(n, { done: true })}>
                          <CheckIcon size={16} />
                        </button>
                      </>
                    )}
                  </span>
                </span>
              </li>
            ))}
          </ul>
        </div>
        {pages > 1 && <Pagination page={page} pages={pages} href={(p) => href(filter, p)} />}
      </div>
    </main>
  );
}
