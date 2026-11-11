"use client";

import { BellIcon, DeviceDesktopIcon, KeyIcon, MoonIcon, PaintbrushIcon, PersonIcon, SunIcon } from "@primer/octicons-react";
import { type FormEvent, type ReactNode, useState } from "react";
import { Container, NotFound } from "@/components/pages/repos";
import { TokensSettings } from "@/components/pages/tokens";
import { Avatar } from "@/components/ui/avatar";
import { Button } from "@/components/ui/button";
import { Field, Flash, TextInput } from "@/components/ui/form";
import { useTitle } from "@/components/ui/misc";
import { api, asApiError, type CurrentUser } from "@/lib/api";
import { Link, Redirect } from "@/lib/router";
import { useSession } from "@/lib/session";
import { type ColorMode, setColorMode, useColorMode } from "@/lib/theme";
import { cn } from "@/lib/utils";

export const SETTINGS_NAV: { section: string | undefined; label: string; icon: ReactNode }[] = [
  { section: undefined, label: "Public profile", icon: <PersonIcon size={16} /> },
  { section: "appearance", label: "Appearance", icon: <PaintbrushIcon size={16} /> },
  { section: "notifications", label: "Notifications", icon: <BellIcon size={16} /> },
  { section: "tokens", label: "Access tokens", icon: <KeyIcon size={16} /> },
];

export function SettingsLayout({ section, nav, children }: { section?: string; nav: typeof SETTINGS_NAV; children: ReactNode }) {
  return (
    <Container>
      <div className="flex flex-col gap-6 md:flex-row">
        <nav aria-label="Settings" className="flex shrink-0 gap-1 overflow-x-auto md:w-[256px] md:flex-col md:pl-2">
          {nav.map((n) => {
            const active = n.section === section;
            return (
              <Link
                key={n.label}
                href={n.section ? `/settings/${n.section}` : "/settings"}
                aria-current={active ? "page" : undefined}
                className={cn(
                  "relative flex h-8 items-center gap-2 rounded-md px-2 text-sm whitespace-nowrap hover:bg-control-transparent-hover",
                  active && "bg-control-transparent-hover font-semibold md:before:absolute md:before:-left-2 md:before:h-6 md:before:w-1 md:before:rounded-md md:before:bg-accent-emphasis",
                )}
              >
                <span className="text-fg-muted">{n.icon}</span>
                {n.label}
              </Link>
            );
          })}
        </nav>
        <div className="min-w-0 flex-1">{children}</div>
      </div>
    </Container>
  );
}

export function Subhead({ children }: { children: ReactNode }) {
  return <h2 className="mb-4 border-b border-line-muted pb-2 text-2xl font-normal">{children}</h2>;
}

function Profile({ user, onSaved }: { user: CurrentUser; onSaved: (u: CurrentUser) => void }) {
  const [name, setName] = useState(user.name ?? "");
  const [status, setStatus] = useState<{ ok: boolean; text: string } | null>(null);
  const [busy, setBusy] = useState(false);

  async function submit(e: FormEvent) {
    e.preventDefault();
    setBusy(true);
    setStatus(null);
    try {
      onSaved(await api<CurrentUser>("/user", { method: "PATCH", body: { name } }));
      setStatus({ ok: true, text: "Profile updated." });
    } catch (err) {
      setStatus({ ok: false, text: asApiError(err).summary });
    } finally {
      setBusy(false);
    }
  }

  return (
    <>
      <Subhead>Public profile</Subhead>
      {status && (
        <Flash variant={status.ok ? "success" : "danger"} className="mb-4" onDismiss={() => setStatus(null)}>
          {status.text}
        </Flash>
      )}
      <div className="flex flex-col-reverse gap-6 md:flex-row">
        <form onSubmit={submit} className="flex max-w-xl flex-1 flex-col gap-4">
          <Field label="Name" htmlFor="profile-name" caption="Shown on your profile next to your username.">
            <TextInput id="profile-name" maxLength={255} value={name} onChange={(e) => setName(e.target.value)} />
          </Field>
          <Field label="Username" htmlFor="profile-login" caption="Usernames cannot be changed.">
            <TextInput id="profile-login" value={user.login} disabled readOnly />
          </Field>
          <Field label="Email" htmlFor="profile-email" caption="Used to sign in and for notification emails.">
            <TextInput id="profile-email" value={user.email} disabled readOnly />
          </Field>
          <div>
            <Button type="submit" variant="primary" disabled={busy || name === (user.name ?? "")}>
              Update profile
            </Button>
          </div>
        </form>
        <div>
          <p className="mb-2 text-sm font-semibold">Profile picture</p>
          <Avatar login={user.login} size={120} />
        </div>
      </div>
    </>
  );
}

const MODES: { mode: ColorMode; label: string; hint: string; icon: ReactNode }[] = [
  { mode: "auto", label: "Sync with system", hint: "Follows your device's light or dark setting", icon: <DeviceDesktopIcon size={16} /> },
  { mode: "light", label: "Light", hint: "Always use the light theme", icon: <SunIcon size={16} /> },
  { mode: "dark", label: "Dark", hint: "Always use the dark theme", icon: <MoonIcon size={16} /> },
];

function Appearance() {
  const mode = useColorMode();
  return (
    <>
      <Subhead>Theme preferences</Subhead>
      <p className="mb-4 text-sm text-fg-muted">Choose how Issue Tracker looks to you. The choice is saved in this browser.</p>
      <div role="radiogroup" aria-label="Theme mode" className="grid gap-3 sm:grid-cols-3">
        {MODES.map((m) => (
          <button
            key={m.mode}
            type="button"
            role="radio"
            aria-checked={mode === m.mode}
            onClick={() => setColorMode(m.mode)}
            className={cn("flex cursor-pointer flex-col gap-1 rounded-md border p-4 text-left", mode === m.mode ? "border-[var(--borderColor-accent-emphasis)] bg-accent-muted" : "border-line hover:bg-canvas-muted")}
          >
            <span className="flex items-center gap-2 text-sm font-semibold">
              {m.icon}
              {m.label}
            </span>
            <span className="text-xs text-fg-muted">{m.hint}</span>
          </button>
        ))}
      </div>
    </>
  );
}

function NotificationSettings({ user, onSaved }: { user: CurrentUser; onSaved: (u: CurrentUser) => void }) {
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  async function toggle(on: boolean) {
    setBusy(true);
    setError(null);
    try {
      onSaved(await api<CurrentUser>("/user", { method: "PATCH", body: { email_notifications: on } }));
    } catch (err) {
      setError(asApiError(err).summary);
    } finally {
      setBusy(false);
    }
  }

  return (
    <>
      <Subhead>Notifications</Subhead>
      {error && <Flash className="mb-4">{error}</Flash>}
      <div className="rounded-md border border-line">
        <label className="flex cursor-pointer items-start gap-3 p-4">
          <input type="checkbox" className="mt-1 h-4 w-4" checked={user.email_notifications} disabled={busy} onChange={(e) => toggle(e.target.checked)} />
          <span>
            <span className="block text-sm font-semibold">Email</span>
            <span className="block text-sm text-fg-muted">
              Send an email to {user.email} when you are mentioned or assigned, and for activity on issues you follow. Everything also appears in your{" "}
              <Link href="/notifications" className="text-fg-accent hover:underline">
                inbox
              </Link>
              .
            </span>
          </span>
        </label>
      </div>
      <p className="mt-2 text-xs text-fg-muted">Emails are only sent when the server administrator has configured outgoing mail.</p>
    </>
  );
}

export function UserSettingsPage({ section, extraNav = [], extra }: { section?: string; extraNav?: typeof SETTINGS_NAV; extra?: (user: CurrentUser) => ReactNode }) {
  useTitle("Settings · Issue Tracker");
  const { user, setUser } = useSession();
  if (user === undefined) return null;
  if (user === null) return <Redirect to="/login?return_to=/settings" />;
  const nav = [...SETTINGS_NAV, ...extraNav];
  if (!nav.some((n) => n.section === section)) return <NotFound />;
  return (
    <SettingsLayout section={section} nav={nav}>
      {section === undefined && <Profile user={user} onSaved={setUser} />}
      {section === "appearance" && <Appearance />}
      {section === "notifications" && <NotificationSettings user={user} onSaved={setUser} />}
      {section === "tokens" && <TokensSettings />}
      {extraNav.some((n) => n.section === section) && extra?.(user)}
    </SettingsLayout>
  );
}
