"use client";

import { IssueOpenedIcon } from "@primer/octicons-react";
import { type FormEvent, useState } from "react";
import { Button } from "@/components/ui/button";
import { Field, Flash, TextInput } from "@/components/ui/form";
import { useTitle } from "@/components/ui/misc";
import { api, ApiError, asApiError, type CurrentUser } from "@/lib/api";
import { Link, navigate } from "@/lib/router";
import { useSession } from "@/lib/session";

function AuthFrame({ heading, children, footer }: { heading: string; children: React.ReactNode; footer: React.ReactNode }) {
  return (
    <div className="min-h-screen bg-canvas-inset px-4 pt-8 pb-10 md:bg-canvas">
      <div className="mx-auto w-full max-w-[340px]">
        <div className="flex flex-col items-center pb-6">
          <Link href="/" aria-label="Homepage" className="flex h-12 w-12 items-center justify-center rounded-full bg-fg text-canvas">
            <IssueOpenedIcon size={30} />
          </Link>
          <h1 className="mt-6 text-2xl font-light tracking-tight">{heading}</h1>
        </div>
        {children}
        <p className="mt-4 rounded-md border border-line p-4 text-center text-sm">{footer}</p>
      </div>
    </div>
  );
}

/** Where to go after signing in: `?return_to=/path`, same site only. */
function returnTo(): string {
  const target = new URLSearchParams(window.location.search).get("return_to");
  return target && target.startsWith("/") && !target.startsWith("//") ? target : "/";
}

export function LoginPage() {
  useTitle("Sign in · Issue Tracker");
  const { setUser } = useSession();
  const [login, setLogin] = useState("");
  const [password, setPassword] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  async function submit(e: FormEvent) {
    e.preventDefault();
    setBusy(true);
    setError(null);
    try {
      const user = await api<CurrentUser>("/auth/login", { method: "POST", body: { login, password } });
      await setUser(user);
      navigate(returnTo(), { replace: true });
    } catch (err) {
      setError(asApiError(err).summary);
      setBusy(false);
    }
  }

  return (
    <AuthFrame
      heading="Sign in to Issue Tracker"
      footer={
        <>
          New here?{" "}
          <Link href="/signup" className="text-fg-accent hover:underline">
            Create an account
          </Link>
        </>
      }
    >
      {error && (
        <Flash className="mb-4" onDismiss={() => setError(null)}>
          {error}
        </Flash>
      )}
      <form onSubmit={submit} className="flex flex-col gap-4 rounded-md border border-line-muted bg-canvas-muted p-4">
        <Field label="Username or email address" htmlFor="login_field">
          <TextInput id="login_field" name="login" autoComplete="username" autoCapitalize="off" autoFocus required value={login} onChange={(e) => setLogin(e.target.value)} />
        </Field>
        <Field label="Password" htmlFor="password">
          <TextInput id="password" name="password" type="password" autoComplete="current-password" required value={password} onChange={(e) => setPassword(e.target.value)} />
        </Field>
        <Button type="submit" variant="primary" block disabled={busy}>
          {busy ? "Signing in…" : "Sign in"}
        </Button>
      </form>
    </AuthFrame>
  );
}

export function SignupPage() {
  useTitle("Sign up · Issue Tracker");
  const { setUser } = useSession();
  const [form, setForm] = useState({ login: "", email: "", password: "" });
  const [error, setError] = useState<ApiError | null>(null);
  const [busy, setBusy] = useState(false);
  const set = (key: keyof typeof form) => (e: React.ChangeEvent<HTMLInputElement>) => setForm({ ...form, [key]: e.target.value });

  async function submit(e: FormEvent) {
    e.preventDefault();
    setBusy(true);
    setError(null);
    try {
      const user = await api<CurrentUser>("/auth/signup", { method: "POST", body: form });
      await setUser(user);
      navigate("/", { replace: true });
    } catch (err) {
      setError(asApiError(err));
      setBusy(false);
    }
  }

  return (
    <AuthFrame
      heading="Create your account"
      footer={
        <>
          Already have an account?{" "}
          <Link href="/login" className="text-fg-accent hover:underline">
            Sign in
          </Link>
        </>
      }
    >
      {error && error.errors.length === 0 && (
        <Flash className="mb-4" onDismiss={() => setError(null)}>
          {error.message}
        </Flash>
      )}
      <form onSubmit={submit} className="flex flex-col gap-4 rounded-md border border-line-muted bg-canvas-muted p-4">
        <Field label="Email" htmlFor="email" required error={error?.field("email")}>
          <TextInput id="email" type="email" autoComplete="email" autoFocus required aria-invalid={!!error?.field("email")} value={form.email} onChange={set("email")} />
        </Field>
        <Field label="Password" htmlFor="password" required error={error?.field("password")} caption="Password should be at least 8 characters.">
          <TextInput id="password" type="password" autoComplete="new-password" required aria-invalid={!!error?.field("password")} value={form.password} onChange={set("password")} />
        </Field>
        <Field
          label="Username"
          htmlFor="login"
          required
          error={error?.field("login")}
          caption="Username may only contain alphanumeric characters or single hyphens, and cannot begin or end with a hyphen."
        >
          <TextInput id="login" autoComplete="username" autoCapitalize="off" required aria-invalid={!!error?.field("login")} value={form.login} onChange={set("login")} />
        </Field>
        <Button type="submit" variant="primary" block disabled={busy}>
          {busy ? "Creating account…" : "Create account"}
        </Button>
      </form>
    </AuthFrame>
  );
}
