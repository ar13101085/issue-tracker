"use client";

import { Dialog as Base } from "@base-ui/react/dialog";
import { BellIcon, GearIcon, HomeIcon, LockIcon, PlusIcon, RepoIcon, SearchIcon, ThreeBarsIcon, XIcon } from "@primer/octicons-react";
import { type FormEvent, type ReactNode, useEffect, useState } from "react";
import useSWR from "swr";
import { api, type Repo } from "@/lib/api";
import { DEFAULT_QUERY } from "@/lib/query";
import { Link, navigate } from "@/lib/router";
import { cn } from "@/lib/utils";

const row = "flex h-8 items-center gap-2 rounded-md px-2 text-sm hover:bg-control-transparent-hover";

/** The hamburger button and the navigation drawer it opens. */
export function GlobalNav({ signedIn, buttonClass }: { signedIn: boolean; buttonClass: string }) {
  const [open, setOpen] = useState(false);
  const repos = useSWR<Repo[]>(open && signedIn ? "/user/repos" : null, api);
  const item = (href: string, icon: ReactNode, label: string) => (
    <Link href={href} onClick={() => setOpen(false)} className={row}>
      <span className="text-fg-muted">{icon}</span>
      {label}
    </Link>
  );
  return (
    <Base.Root open={open} onOpenChange={setOpen}>
      <Base.Trigger aria-label="Open global navigation menu" className={buttonClass}>
        <ThreeBarsIcon size={16} />
      </Base.Trigger>
      <Base.Portal>
        <Base.Backdrop className="fixed inset-0 z-50 bg-[var(--overlay-backdrop-bgColor)]" />
        <Base.Popup className="fixed inset-y-0 left-0 z-50 flex w-[320px] max-w-[85vw] flex-col rounded-r-xl bg-popover shadow-[var(--shadow-floating-large)] outline-none">
          <header className="flex items-center justify-between p-4 pb-2">
            <Base.Title className="text-sm font-semibold">Issue Tracker</Base.Title>
            <Base.Close aria-label="Close" className="flex h-8 w-8 cursor-pointer items-center justify-center rounded-md text-fg-muted hover:bg-control-transparent-hover">
              <XIcon size={16} />
            </Base.Close>
          </header>
          <nav aria-label="Global" className="min-h-0 flex-1 overflow-y-auto px-2 pb-4">
            {item("/", <HomeIcon size={16} />, "Home")}
            {signedIn && item("/notifications", <BellIcon size={16} />, "Notifications")}
            {signedIn && item("/settings", <GearIcon size={16} />, "Settings")}
            {signedIn && (
              <>
                <div className="mx-2 my-2 h-px bg-line-muted" />
                <div className="flex items-center justify-between px-2 py-1 text-xs font-semibold text-fg-muted">
                  Repositories
                  <Link href="/new" onClick={() => setOpen(false)} aria-label="New repository" className="flex h-6 w-6 items-center justify-center rounded-md hover:bg-control-transparent-hover">
                    <PlusIcon size={16} />
                  </Link>
                </div>
                {repos.data?.length === 0 && <p className="px-2 py-1 text-sm text-fg-muted">No repositories yet.</p>}
                {repos.data?.map((r) => (
                  <Link key={`${r.owner}/${r.name}`} href={`/${r.owner}/${r.name}`} onClick={() => setOpen(false)} className={row}>
                    <span className="text-fg-muted">{r.private ? <LockIcon size={16} /> : <RepoIcon size={16} />}</span>
                    <span className="truncate">
                      {r.owner}/<span className="font-semibold">{r.name}</span>
                    </span>
                  </Link>
                ))}
              </>
            )}
            {!signedIn && (
              <>
                <div className="mx-2 my-2 h-px bg-line-muted" />
                {item("/login", <GearIcon size={16} />, "Sign in")}
              </>
            )}
          </nav>
        </Base.Popup>
      </Base.Portal>
    </Base.Root>
  );
}

/** The "Type / to search" box. Searches the current repository's issues, or jumps to one of your repositories. */
export function HeaderSearch({ repo, signedIn, iconClass }: { repo: string | null; signedIn: boolean; iconClass: string }) {
  const [open, setOpen] = useState(false);
  const [text, setText] = useState("");
  const repos = useSWR<Repo[]>(open && signedIn ? "/user/repos" : null, api);

  // "/" opens the search unless the user is typing somewhere.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const el = e.target as HTMLElement | null;
      const typing = !!el && (el.tagName === "INPUT" || el.tagName === "TEXTAREA" || el.tagName === "SELECT" || el.isContentEditable);
      if (e.key === "/" && !typing && !e.metaKey && !e.ctrlKey && !e.altKey) {
        e.preventDefault();
        setOpen(true);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  function go(href: string) {
    setOpen(false);
    setText("");
    navigate(href);
  }

  function submit(e: FormEvent) {
    e.preventDefault();
    const q = text.trim();
    if (repo) go(q ? `/${repo}/issues?q=${encodeURIComponent(`${DEFAULT_QUERY} ${q}`)}` : `/${repo}/issues`);
    else if (matches[0]) go(`/${matches[0].owner}/${matches[0].name}`);
  }

  const needle = text.trim().toLowerCase();
  const matches = (repos.data ?? []).filter((r) => !needle || `${r.owner}/${r.name}`.toLowerCase().includes(needle)).slice(0, 8);

  return (
    <Base.Root open={open} onOpenChange={setOpen}>
      <Base.Trigger aria-label="Search" className={cn(iconClass, "lg:hidden")}>
        <SearchIcon size={16} />
      </Base.Trigger>
      <Base.Trigger className="hidden h-8 w-[197px] cursor-pointer items-center gap-2 rounded-md border border-line px-3 text-left text-sm text-fg-muted lg:flex">
        <SearchIcon size={16} />
        <span className="flex-1">
          Type <kbd className="inline-grid h-4 w-4 place-items-center rounded-[3px] border border-[var(--fgColor-muted)] align-middle text-xs leading-none">/</kbd> to search
        </span>
      </Base.Trigger>
      <Base.Portal>
        <Base.Backdrop className="fixed inset-0 z-50 bg-[var(--overlay-backdrop-bgColor)]" />
        <Base.Popup className="fixed inset-x-2 top-2 z-50 mx-auto max-w-[640px] rounded-xl bg-popover p-2 shadow-[var(--shadow-floating-large)] outline-none md:top-4">
          <Base.Title className="sr-only">Search</Base.Title>
          <form onSubmit={submit} className="flex items-center gap-2 rounded-md border border-fg-accent px-2">
            <SearchIcon size={16} className="text-fg-muted" />
            <input
              autoFocus
              value={text}
              onChange={(e) => setText(e.target.value)}
              aria-label={repo ? `Search issues in ${repo}` : "Search your repositories"}
              placeholder={repo ? `Search issues in ${repo}` : "Search your repositories"}
              className="h-9 min-w-0 flex-1 bg-transparent text-sm outline-none"
            />
          </form>
          <ul className="mt-2">
            {repo && (
              <li>
                <button type="button" onClick={submit} className={cn(row, "w-full cursor-pointer text-left")}>
                  <SearchIcon size={16} className="shrink-0 text-fg-muted" />
                  <span className="truncate">{needle ? <>Search issues for “{text.trim()}”</> : <>All open issues</>}</span>
                  <span className="ml-auto shrink-0 text-xs text-fg-muted">in {repo}</span>
                </button>
              </li>
            )}
            {matches.map((r) => (
              <li key={`${r.owner}/${r.name}`}>
                <button type="button" onClick={() => go(`/${r.owner}/${r.name}`)} className={cn(row, "w-full cursor-pointer text-left")}>
                  <span className="shrink-0 text-fg-muted">{r.private ? <LockIcon size={16} /> : <RepoIcon size={16} />}</span>
                  <span className="truncate">
                    {r.owner}/{r.name}
                  </span>
                  <span className="ml-auto shrink-0 text-xs text-fg-muted">Jump to</span>
                </button>
              </li>
            ))}
            {!repo && signedIn && repos.data && matches.length === 0 && <li className="px-2 py-2 text-sm text-fg-muted">No repositories match.</li>}
            {!repo && !signedIn && <li className="px-2 py-2 text-sm text-fg-muted">Sign in to search your repositories.</li>}
          </ul>
        </Base.Popup>
      </Base.Portal>
    </Base.Root>
  );
}
