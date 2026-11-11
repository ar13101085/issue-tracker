"use client";

import { Popover } from "@base-ui/react/popover";
import { SmileyIcon } from "@primer/octicons-react";
import { useState } from "react";
import { api, REACTIONS, type ReactionGroup } from "@/lib/api";
import { cn } from "@/lib/utils";

const EMOJI = Object.fromEntries(REACTIONS.map((r) => [r.content, r]));

function who(group: ReactionGroup): string {
  const names = group.users.slice(0, 5).join(", ");
  const more = group.count - Math.min(5, group.users.length);
  return `${names}${more > 0 ? ` and ${more} more` : ""} reacted with ${EMOJI[group.content]?.label.toLowerCase() ?? group.content}`;
}

/** The emoji row under a comment. `path` is the reactions URL of the issue or comment. */
export function Reactions({ groups, path, canReact, onChange }: { groups: ReactionGroup[]; path: string; canReact: boolean; onChange: () => void }) {
  const [open, setOpen] = useState(false);
  const [busy, setBusy] = useState(false);

  async function toggle(content: string) {
    if (busy) return;
    setBusy(true);
    setOpen(false);
    const mine = groups.find((g) => g.content === content)?.viewer_reacted;
    try {
      await api(`${path}/${encodeURIComponent(content)}`, { method: mine ? "DELETE" : "PUT" });
      onChange();
    } catch {
      // The row is refetched either way; a failed toggle simply does not show.
      onChange();
    } finally {
      setBusy(false);
    }
  }

  if (!canReact && groups.length === 0) return null;
  const pill = "inline-flex h-7 items-center gap-1 rounded-full border px-2 text-xs";
  return (
    <div className="mt-4 flex flex-wrap items-center gap-1">
      {canReact && (
        <Popover.Root open={open} onOpenChange={setOpen}>
          <Popover.Trigger
            render={
              <button type="button" aria-label="Add or remove reactions" className={cn(pill, "w-7 cursor-pointer justify-center border-line bg-canvas-muted px-0 text-fg-muted hover:bg-control-hover")}>
                <SmileyIcon size={16} />
              </button>
            }
          />
          <Popover.Portal>
            <Popover.Positioner align="start" sideOffset={4} className="z-50">
              <Popover.Popup className="flex gap-0.5 rounded-xl bg-popover p-1 shadow-[var(--shadow-floating-small)] outline-none">
                {REACTIONS.map((r) => {
                  const mine = groups.find((g) => g.content === r.content)?.viewer_reacted;
                  return (
                    <button
                      key={r.content}
                      type="button"
                      aria-label={r.label}
                      aria-pressed={!!mine}
                      title={r.label}
                      onClick={() => toggle(r.content)}
                      className={cn("flex h-8 w-8 cursor-pointer items-center justify-center rounded-md text-base hover:bg-control-transparent-hover", mine && "bg-accent-muted")}
                    >
                      {r.emoji}
                    </button>
                  );
                })}
              </Popover.Popup>
            </Popover.Positioner>
          </Popover.Portal>
        </Popover.Root>
      )}
      {groups.map((g) => (
        <button
          key={g.content}
          type="button"
          disabled={!canReact}
          title={who(g)}
          aria-label={who(g)}
          aria-pressed={g.viewer_reacted}
          onClick={() => toggle(g.content)}
          className={cn(
            pill,
            canReact && "cursor-pointer",
            g.viewer_reacted ? "border-[var(--borderColor-accent-emphasis)] bg-accent-muted text-fg-accent" : "border-line bg-canvas text-fg-muted hover:bg-canvas-muted",
          )}
        >
          <span className="text-sm">{EMOJI[g.content]?.emoji ?? g.content}</span>
          {g.count}
        </button>
      ))}
    </div>
  );
}
