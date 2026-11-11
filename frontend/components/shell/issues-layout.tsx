"use client";

import { BookmarkIcon, ClockIcon, IssueOpenedIcon, MentionIcon, MilestoneIcon, PersonIcon, SmileyIcon, TableIcon, TagIcon } from "@primer/octicons-react";
import type { ReactNode } from "react";
import { DEFAULT_QUERY, searchHref, setToken } from "@/lib/query";
import { Link } from "@/lib/router";
import { useSession } from "@/lib/session";
import { cn } from "@/lib/utils";

export type IssuesSection = "issues" | "assigned" | "created" | "mentioned" | "recent" | "views" | "projects" | "milestones" | "labels";

export const assignedQuery = setToken(DEFAULT_QUERY, "assignee", "@me");
export const createdQuery = setToken(DEFAULT_QUERY, "author", "@me");
export const mentionedQuery = setToken(DEFAULT_QUERY, "mentions", "@me");
export const recentQuery = "is:issue involves:@me sort:updated-desc";

/** The 256px navigation on the left of every issues page (issues, projects, milestones, labels). */
export function IssuesNav({ owner, repo, active }: { owner: string; repo: string; active: IssuesSection }) {
  const { user } = useSession();
  const base = `/${owner}/${repo}`;
  const item = (key: IssuesSection, href: string, icon: ReactNode, label: string) => (
    <li>
      <Link
        href={href}
        aria-current={active === key ? "page" : undefined}
        className={cn(
          "relative flex h-8 items-center gap-2 rounded-md px-2 text-sm hover:bg-control-transparent-hover",
          active === key &&
            "bg-[var(--control-transparent-bgColor-selected)] font-semibold before:absolute before:top-1 before:bottom-1 before:-left-2 before:w-1 before:rounded-md before:bg-accent-emphasis",
        )}
      >
        <span className="text-fg-muted">{icon}</span>
        {label}
      </Link>
    </li>
  );
  return (
    <aside className="hidden w-[256px] shrink-0 border-r border-line p-2 lg:block">
      <nav aria-label="Issues">
        <ul className="px-2 py-2">
          {item("issues", `${base}/issues`, <IssueOpenedIcon size={16} />, "Issues")}
          {user && item("assigned", searchHref(`${base}/issues`, assignedQuery), <PersonIcon size={16} />, "Assigned to me")}
          {user && item("created", searchHref(`${base}/issues`, createdQuery), <SmileyIcon size={16} />, "Created by me")}
          {user && item("mentioned", searchHref(`${base}/issues`, mentionedQuery), <MentionIcon size={16} />, "Mentioned")}
          {user && item("recent", searchHref(`${base}/issues`, recentQuery), <ClockIcon size={16} />, "Recent activity")}
        </ul>
        <ul className="mx-2 border-t border-line-muted py-2">
          {item("views", `${base}/issues/views`, <BookmarkIcon size={16} />, "Views")}
          {item("projects", `${base}/projects`, <TableIcon size={16} />, "Projects")}
          {item("milestones", `${base}/milestones`, <MilestoneIcon size={16} />, "Milestones")}
          {item("labels", `${base}/labels`, <TagIcon size={16} />, "Labels")}
        </ul>
      </nav>
    </aside>
  );
}

/** Sidebar plus the page frame GitHub uses for labels, milestones and projects: 24px gutters, 20px heading. */
export function IssuesLayout({
  owner,
  repo,
  active,
  title,
  actions,
  children,
}: {
  owner: string;
  repo: string;
  active: IssuesSection;
  title: string;
  actions?: ReactNode;
  children: ReactNode;
}) {
  const base = `/${owner}/${repo}`;
  const small = (key: IssuesSection, href: string, label: string) =>
    active !== key && (
      <Link href={href} className="text-fg-accent hover:underline">
        {label}
      </Link>
    );
  return (
    <div className="lg:flex">
      <IssuesNav owner={owner} repo={repo} active={active} />
      <main className="min-w-0 flex-1 px-4 pt-4 pb-6 md:px-6 md:pt-6">
        <div className="flex min-h-[33px] items-center justify-between gap-2">
          <h1 className="text-xl leading-[32.5px] font-semibold">{title}</h1>
          <div className="flex items-center gap-2">{actions}</div>
        </div>
        {/* Below lg the sidebar is hidden, so the sibling pages are linked here. */}
        <p className="mt-2 flex gap-4 text-sm lg:hidden">
          {small("issues", `${base}/issues`, "Issues")}
          {small("views", `${base}/issues/views`, "Views")}
          {small("projects", `${base}/projects`, "Projects")}
          {small("milestones", `${base}/milestones`, "Milestones")}
          {small("labels", `${base}/labels`, "Labels")}
        </p>
        <div className="pt-4">{children}</div>
      </main>
    </div>
  );
}

/** Bordered list with the 48px grey header GitHub puts on labels, milestones and projects. */
export function ListBox({ header, children }: { header: ReactNode; children: ReactNode }) {
  return (
    <div className="rounded-md border border-line">
      <div className="flex min-h-12 items-center justify-between gap-2 rounded-t-md border-b border-line bg-canvas-muted p-2">{header}</div>
      {children}
    </div>
  );
}

/** Tabs inside a ListBox header: "Open 7  Closed 407". */
export function ListTabs<T extends string>({ tabs, value, onChange }: { tabs: { key: T; label: string; count: number }[]; value: T; onChange?: (key: T) => void }) {
  return (
    <ul className="flex">
      {tabs.map((t) => (
        <li key={t.key}>
          <button
            type="button"
            aria-current={value === t.key ? "true" : undefined}
            onClick={() => onChange?.(t.key)}
            className={cn("flex h-[30px] cursor-pointer items-center rounded-md px-2 text-sm hover:bg-control-transparent-hover", value === t.key ? "font-semibold text-fg" : "text-fg-muted")}
          >
            {t.label}
            <span className="ml-2 inline-flex h-5 min-w-8 items-center justify-center rounded-full bg-neutral-muted px-[7px] text-xs font-semibold text-fg">{t.count}</span>
          </button>
        </li>
      ))}
    </ul>
  );
}
