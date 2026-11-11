"use client";

import {
  BellIcon,
  GearIcon,
  IssueOpenedIcon,
  PlusIcon,
  RepoIcon,
  SignOutIcon,
  TableIcon,
  ThreeBarsIcon,
  TriangleDownIcon,
} from "@primer/octicons-react";
import type { ReactNode } from "react";
import useSWR, { useSWRConfig } from "swr";
import { ActionDivider, ActionHeading, ActionItem, ActionMenu } from "@/components/ui/action-menu";
import { GlobalNav, HeaderSearch } from "@/components/shell/global-nav";
import { Avatar } from "@/components/ui/avatar";
import { buttonClass } from "@/components/ui/button";
import { Counter } from "@/components/ui/misc";
import { api, hasRole, type NotificationList } from "@/lib/api";
import { Link, navigate, type Route } from "@/lib/router";
import { useRepo, useSession } from "@/lib/session";
import { cn } from "@/lib/utils";

// 32px square button used all over the global bar.
const iconButton =
  "inline-flex h-8 w-8 shrink-0 cursor-pointer items-center justify-center rounded-md border border-line text-fg-muted hover:bg-control-transparent-hover";

function Logo() {
  return (
    <Link href="/" aria-label="Homepage" className="flex h-8 w-8 shrink-0 items-center justify-center rounded-full bg-fg text-canvas">
      <IssueOpenedIcon size={20} />
    </Link>
  );
}

function Crumb({ href, bold, children }: { href: string; bold?: boolean; children: ReactNode }) {
  return (
    <Link href={href} className={cn("min-w-0 truncate rounded-md px-1.5 py-1 text-sm text-fg hover:bg-control-transparent-hover", bold && "font-semibold")}>
      {children}
    </Link>
  );
}

function repoOf(route: Route): { owner: string; repo?: string } | null {
  if ("repo" in route) return { owner: route.owner, repo: route.repo };
  if (route.name === "owner") return { owner: route.owner };
  return null;
}

export function AppHeader({ route }: { route: Route }) {
  const ctx = repoOf(route);
  const { user, setUser } = useSession();
  const { mutate } = useSWRConfig();
  const { data: repo } = useRepo(ctx?.owner, ctx?.repo);
  const inbox = useSWR<NotificationList>(user ? "/notifications?filter=unread&page=1" : null, api);
  const unread = inbox.data?.unread_count ?? 0;
  const title = route.name === "notifications" ? "Notifications" : route.name === "home" ? "Dashboard" : route.name === "new-repo" ? "New repository" : route.name === "user-settings" ? "Settings" : null;

  async function signOut() {
    await api("/auth/logout", { method: "POST" });
    await setUser(null);
    // Drop everything fetched as the old account.
    mutate(() => true, undefined, { revalidate: true });
    navigate("/");
  }

  return (
    <header className="bg-canvas-inset shadow-[inset_0_-1px_var(--borderColor-default)]">
      <div className="flex items-center gap-3 p-4">
        <div className="flex min-w-0 flex-1 items-center gap-2">
          <GlobalNav signedIn={!!user} buttonClass={iconButton} />
          <Logo />
          <nav aria-label="Page context" className="flex min-w-0 items-center">
            {ctx ? (
              <>
                <Crumb href={`/${ctx.owner}`} bold={!ctx.repo}>
                  {ctx.owner}
                </Crumb>
                {ctx.repo && (
                  <>
                    <span className="text-fg-muted">/</span>
                    <Crumb href={`/${ctx.owner}/${ctx.repo}`} bold>
                      {ctx.repo}
                    </Crumb>
                  </>
                )}
              </>
            ) : (
              title && <span className="px-1.5 py-1 text-sm font-semibold">{title}</span>
            )}
          </nav>
        </div>

        <div className="flex shrink-0 items-center gap-2">
          <HeaderSearch repo={ctx?.repo ? `${ctx.owner}/${ctx.repo}` : null} signedIn={!!user} iconClass={iconButton} />

          {user && (
            <>
              <span className="mx-1 hidden h-5 w-px bg-line md:block" />
              <ActionMenu
                trigger={
                  <button
                    type="button"
                    aria-label="Create new"
                    className="hidden h-8 cursor-pointer items-center gap-1 rounded-md border border-line px-2 text-fg-muted hover:bg-control-transparent-hover md:inline-flex"
                  >
                    <PlusIcon size={16} />
                    <TriangleDownIcon size={16} />
                  </button>
                }
              >
                {ctx?.repo && (
                  <ActionItem leading={<IssueOpenedIcon size={16} />} onSelect={() => navigate(`/${ctx.owner}/${ctx.repo}/issues/new`)}>
                    New issue
                  </ActionItem>
                )}
                <ActionItem leading={<RepoIcon size={16} />} onSelect={() => navigate("/new")}>
                  New repository
                </ActionItem>
              </ActionMenu>
              <Link href="/notifications" aria-label={unread ? `Notifications, ${unread} unread` : "Notifications"} className={cn(iconButton, "relative")}>
                <BellIcon size={16} />
                {unread > 0 && <span className="absolute -top-1 -right-1 h-3.5 w-3.5 rounded-full border-2 border-[var(--bgColor-inset)] bg-accent-emphasis" />}
              </Link>
              <ActionMenu
                width={256}
                trigger={
                  <button type="button" aria-label="Open user navigation menu" className="cursor-pointer rounded-full">
                    <Avatar login={user.login} size={32} />
                  </button>
                }
              >
                <ActionHeading>
                  Signed in as <span className="text-fg">{user.login}</span>
                </ActionHeading>
                <ActionDivider />
                <ActionItem leading={<RepoIcon size={16} />} onSelect={() => navigate("/")}>
                  Your repositories
                </ActionItem>
                <ActionItem leading={<GearIcon size={16} />} onSelect={() => navigate("/settings")}>
                  Settings
                </ActionItem>
                <ActionDivider />
                <ActionItem leading={<SignOutIcon size={16} />} onSelect={signOut}>
                  Sign out
                </ActionItem>
              </ActionMenu>
            </>
          )}
          {user === null && (
            <>
              <Link href="/login" className={buttonClass("default")}>
                Sign in
              </Link>
              <Link href="/signup" className={cn(buttonClass("default"), "hidden sm:inline-flex")}>
                Sign up
              </Link>
            </>
          )}
        </div>
      </div>

      {ctx?.repo && <RepoNav owner={ctx.owner} repo={ctx.repo} route={route} openIssues={repo?.open_issues_count} admin={hasRole(repo, "admin")} />}
    </header>
  );
}

function RepoNav({ owner, repo, route, openIssues, admin }: { owner: string; repo: string; route: Route; openIssues?: number; admin: boolean }) {
  const base = `/${owner}/${repo}`;
  const tabs = [
    {
      label: "Issues",
      href: `${base}/issues`,
      icon: <IssueOpenedIcon size={16} />,
      count: openIssues,
      active: ["issues", "issue", "issue-new", "labels", "milestones", "milestone-new", "milestone-edit"].includes(route.name),
    },
    { label: "Projects", href: `${base}/projects`, icon: <TableIcon size={16} />, active: route.name === "projects" || route.name === "project" },
    ...(admin ? [{ label: "Settings", href: `${base}/settings`, icon: <GearIcon size={16} />, active: route.name === "settings" }] : []),
  ];
  return (
    <nav aria-label="Repository" className="overflow-x-auto px-4 [scrollbar-width:none]">
      <ul className="flex min-h-12 items-center gap-2">
        {tabs.map((t) => (
          <li key={t.label} className="relative flex h-12 items-center">
            <Link
              href={t.href}
              aria-current={t.active ? "page" : undefined}
              className={cn("flex h-8 items-center gap-2 rounded-md px-2 text-sm whitespace-nowrap text-fg hover:bg-control-transparent-hover", t.active && "font-semibold")}
            >
              <span className="text-fg-muted">{t.icon}</span>
              {t.label}
              {"count" in t && !!t.count && <Counter>{t.count > 5000 ? "5k+" : t.count}</Counter>}
            </Link>
            {t.active && <span className="absolute inset-x-0 bottom-0 h-0.5 rounded-md bg-nav-active" />}
          </li>
        ))}
      </ul>
    </nav>
  );
}

// Prerendered into the static HTML: same height and colors as the real header,
// so nothing jumps when the app takes over.
export function HeaderSkeleton() {
  return (
    <header className="bg-canvas-inset shadow-[inset_0_-1px_var(--borderColor-default)]">
      <div className="flex items-center gap-2 p-4">
        <span className={iconButton}>
          <ThreeBarsIcon size={16} />
        </span>
        <span className="flex h-8 w-8 items-center justify-center rounded-full bg-fg text-canvas">
          <IssueOpenedIcon size={20} />
        </span>
      </div>
    </header>
  );
}
