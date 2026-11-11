"use client";

import { CheckIcon, CopyIcon, KeyIcon } from "@primer/octicons-react";
import { type FormEvent, useState } from "react";
import useSWR from "swr";
import { Subhead } from "@/components/pages/user-settings";
import { Blankslate } from "@/components/shell/blankslate";
import { Button } from "@/components/ui/button";
import { Field, Flash, TextInput } from "@/components/ui/form";
import { RelativeTime } from "@/components/ui/misc";
import { api, ApiError, asApiError } from "@/lib/api";

type Token = { id: string; name: string; prefix: string; created_at: string; last_used_at: string | null };

/** Settings → Access tokens: what the `it` CLI and scripts sign in with. */
export function TokensSettings() {
  const { data, error, mutate } = useSWR<Token[], ApiError>("/user/tokens", api);
  const [name, setName] = useState("");
  const [fresh, setFresh] = useState<{ name: string; token: string } | null>(null);
  const [copied, setCopied] = useState(false);
  const [failure, setFailure] = useState<ApiError | null>(null);
  const [busy, setBusy] = useState(false);

  async function create(e: FormEvent) {
    e.preventDefault();
    setBusy(true);
    setFailure(null);
    try {
      const made = await api<Token & { token: string }>("/user/tokens", { method: "POST", body: { name } });
      setFresh({ name: made.name, token: made.token });
      setCopied(false);
      setName("");
      await mutate();
    } catch (err) {
      setFailure(asApiError(err));
    } finally {
      setBusy(false);
    }
  }

  async function revoke(token: Token) {
    setFailure(null);
    try {
      await api(`/user/tokens/${token.id}`, { method: "DELETE" });
      await mutate();
    } catch (err) {
      setFailure(asApiError(err));
    }
  }

  return (
    <>
      <Subhead>Access tokens</Subhead>
      <p className="mb-4 text-sm text-fg-muted">
        A token lets the <code className="rounded bg-neutral-muted px-1 font-mono text-xs">it</code> command line tool, or a script, act as you without your password. Treat it like a password: anyone holding it can
        do what you can.
      </p>
      {(failure || error) && failure?.errors.length !== 1 && <Flash className="mb-4">{(failure ?? error)?.summary}</Flash>}

      {fresh && (
        <Flash variant="success" className="mb-4" onDismiss={() => setFresh(null)}>
          <p className="font-semibold">Copy the token for “{fresh.name}” now. It will not be shown again.</p>
          <div className="mt-2 flex flex-wrap items-center gap-2">
            <code className="min-w-0 flex-1 rounded-md border border-line bg-canvas px-2 py-1 font-mono text-xs break-all">{fresh.token}</code>
            <Button
              size="sm"
              leading={copied ? <CheckIcon size={16} /> : <CopyIcon size={16} />}
              onClick={() => navigator.clipboard?.writeText(fresh.token).then(() => setCopied(true))}
            >
              {copied ? "Copied" : "Copy"}
            </Button>
          </div>
        </Flash>
      )}

      <form onSubmit={create} className="mb-6 flex flex-wrap items-end gap-2">
        <div className="min-w-[220px] flex-1">
          <Field label="New token" htmlFor="token-name" error={failure?.field("name")} caption="A note to recognise it by, such as where it is used.">
            <TextInput id="token-name" maxLength={100} placeholder="Work laptop" value={name} onChange={(e) => setName(e.target.value)} />
          </Field>
        </div>
        <Button type="submit" variant="primary" className="mb-5" disabled={busy || !name.trim()}>
          Generate token
        </Button>
      </form>

      <div className="rounded-md border border-line">
        <div className="rounded-t-md border-b border-line bg-canvas-muted px-4 py-3 text-sm font-semibold">{data ? `${data.length} token${data.length === 1 ? "" : "s"}` : "Tokens"}</div>
        {data && data.length === 0 && <Blankslate icon={<KeyIcon size={24} />} title="No tokens yet" description="Generate one here, or run “it auth login” to create one from the terminal." />}
        <ul className="divide-y divide-line-muted">
          {data?.map((t) => (
            <li key={t.id} className="flex flex-wrap items-center gap-3 px-4 py-3">
              <KeyIcon size={16} className="text-fg-muted" />
              <div className="min-w-0 flex-1">
                <p className="truncate text-sm font-semibold">{t.name}</p>
                <p className="text-xs text-fg-muted">
                  <code className="font-mono">{t.prefix}…</code> · created <RelativeTime iso={t.created_at} /> ·{" "}
                  {t.last_used_at ? (
                    <>
                      last used <RelativeTime iso={t.last_used_at} />
                    </>
                  ) : (
                    "never used"
                  )}
                </p>
              </div>
              <Button variant="danger" size="sm" aria-label={`Revoke ${t.name}`} onClick={() => revoke(t)}>
                Revoke
              </Button>
            </li>
          ))}
        </ul>
      </div>
    </>
  );
}
