"use client";

// Client-side router. The app is a static export served as one shell for every URL,
// so paths like /owner/repo/issues/12 are matched here instead of by Next's file routes.
// history.pushState is picked up by Next's usePathname/useSearchParams.

import { usePathname } from "next/navigation";
import { type AnchorHTMLAttributes, type MouseEvent, useEffect } from "react";

export type Route =
  | { name: "home" }
  | { name: "login" }
  | { name: "signup" }
  | { name: "notifications" }
  | { name: "new-repo" }
  | { name: "owner"; owner: string }
  | { name: "issues"; owner: string; repo: string }
  | { name: "issue-new"; owner: string; repo: string }
  | { name: "issue"; owner: string; repo: string; number: number }
  | { name: "labels"; owner: string; repo: string }
  | { name: "views"; owner: string; repo: string }
  | { name: "view-new"; owner: string; repo: string }
  | { name: "view-edit"; owner: string; repo: string; id: string }
  | { name: "milestones"; owner: string; repo: string }
  | { name: "milestone-new"; owner: string; repo: string }
  | { name: "milestone-edit"; owner: string; repo: string; number: number }
  | { name: "project"; owner: string; repo: string; number: number }
  | { name: "user-settings"; section?: string }
  | { name: "projects"; owner: string; repo: string }
  | { name: "settings"; owner: string; repo: string; section?: string }
  | { name: "not-found" };

// First path segments that can never be a user name.
export const RESERVED = ["login", "signup", "logout", "notifications", "new", "settings", "api", "_next"];

export function matchRoute(pathname: string): Route {
  const parts = pathname.split("/").filter(Boolean).map(decodeURIComponent);
  const [a, b, c, d] = parts;
  if (parts.length === 0) return { name: "home" };
  if (parts.length === 1) {
    if (a === "login") return { name: "login" };
    if (a === "signup") return { name: "signup" };
    if (a === "notifications") return { name: "notifications" };
    if (a === "new") return { name: "new-repo" };
    if (a === "settings") return { name: "user-settings" };
    if (RESERVED.includes(a)) return { name: "not-found" };
    return { name: "owner", owner: a };
  }
  if (a === "settings" && parts.length === 2) return { name: "user-settings", section: b };
  if (RESERVED.includes(a)) return { name: "not-found" };
  const base = { owner: a, repo: b };
  if (parts.length === 2) return { name: "issues", ...base };
  if (c === "issues") {
    if (parts.length === 3) return { name: "issues", ...base };
    if (parts.length === 4 && d === "new") return { name: "issue-new", ...base };
    if (d === "views") {
      if (parts.length === 4) return { name: "views", ...base };
      if (parts.length === 5 && parts[4] === "new") return { name: "view-new", ...base };
      if (parts.length === 6 && /^[0-9a-f]{24}$/.test(parts[4]) && parts[5] === "edit") return { name: "view-edit", ...base, id: parts[4] };
    }
    if (parts.length === 4 && /^[1-9]\d*$/.test(d)) return { name: "issue", ...base, number: Number(d) };
  }
  if (c === "labels" && parts.length === 3) return { name: "labels", ...base };
  if (c === "milestones" && parts.length === 3) return { name: "milestones", ...base };
  if (c === "milestones" && parts.length === 4 && d === "new") return { name: "milestone-new", ...base };
  if (c === "milestones" && parts.length === 5 && /^[1-9]\d*$/.test(d) && parts[4] === "edit") return { name: "milestone-edit", ...base, number: Number(d) };
  if (c === "projects" && parts.length === 3) return { name: "projects", ...base };
  if (c === "projects" && parts.length === 4 && /^[1-9]\d*$/.test(d)) return { name: "project", ...base, number: Number(d) };
  if (c === "settings" && parts.length <= 4) return { name: "settings", ...base, section: d };
  return { name: "not-found" };
}

export function useRoute(): Route {
  return matchRoute(usePathname());
}

export function navigate(href: string, opts: { replace?: boolean } = {}) {
  if (opts.replace) window.history.replaceState(null, "", href);
  else window.history.pushState(null, "", href);
  if (!href.includes("#")) window.scrollTo(0, 0);
}

export function Link({ href, onClick, ...rest }: AnchorHTMLAttributes<HTMLAnchorElement> & { href: string }) {
  function handle(e: MouseEvent<HTMLAnchorElement>) {
    onClick?.(e);
    if (e.defaultPrevented || e.button !== 0 || e.metaKey || e.ctrlKey || e.shiftKey || e.altKey) return;
    if (rest.target && rest.target !== "_self") return;
    if (!href.startsWith("/") || href.startsWith("//")) return;
    e.preventDefault();
    navigate(href);
  }
  return <a href={href} onClick={handle} {...rest} />;
}

/** Renders nothing and replaces the current history entry with `to`. */
export function Redirect({ to }: { to: string }) {
  useEffect(() => navigate(to, { replace: true }), [to]);
  return null;
}
