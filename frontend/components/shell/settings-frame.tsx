"use client";

import type { ReactNode } from "react";
import { Link } from "@/lib/router";
import { cn } from "@/lib/utils";

export type SettingsNavItem = { href: string; active: boolean; icon: ReactNode; label: string };

/**
 * Settings pages: a 256px navigation and the content beside it, 40px apart.
 * `header` is the account banner shown above both on personal settings.
 */
export function SettingsFrame({ nav, header, children }: { nav: SettingsNavItem[]; header?: ReactNode; children: ReactNode }) {
  return (
    <main className={cn("mx-auto w-full px-4 pb-10", header ? "max-w-[1280px] pt-6" : "pt-6 md:px-8")}>
      {header}
      <div className={cn("grid grid-cols-[minmax(0,1fr)] gap-y-4", header ? "gap-x-6 md:grid-cols-[296px_minmax(0,1fr)]" : "gap-x-10 md:grid-cols-[256px_minmax(0,1fr)]")}>
        <nav aria-label="Settings">
          <ul className="flex gap-1 overflow-x-auto md:block">
            {nav.map((n) => (
              <li key={n.href}>
                <Link
                  href={n.href}
                  aria-current={n.active ? "page" : undefined}
                  className={cn(
                    "relative flex h-[33px] items-center gap-2 rounded-md px-2 text-sm whitespace-nowrap hover:bg-control-transparent-hover",
                    n.active &&
                      "bg-[var(--control-transparent-bgColor-selected)] font-semibold md:before:absolute md:before:top-1 md:before:bottom-1 md:before:-left-2 md:before:w-1 md:before:rounded-md md:before:bg-accent-emphasis",
                  )}
                >
                  <span className="flex w-4 justify-center text-fg-muted">{n.icon}</span>
                  {n.label}
                </Link>
              </li>
            ))}
          </ul>
        </nav>
        <div className="min-w-0">{children}</div>
      </div>
    </main>
  );
}

/** Section heading: 24px, with the hairline under it. */
export function Subhead({ children, danger }: { children: ReactNode; danger?: boolean }) {
  return <h2 className={cn("mb-4 border-b border-line-muted pb-2 text-2xl leading-9 font-normal", danger && "border-0 pb-0 text-fg-danger")}>{children}</h2>;
}
