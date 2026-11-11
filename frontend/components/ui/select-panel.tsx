"use client";

import { Popover } from "@base-ui/react/popover";
import { CheckIcon, XIcon } from "@primer/octicons-react";
import { type ReactElement, type ReactNode, useEffect, useState } from "react";
import { cn } from "@/lib/utils";

export type PanelItem = {
  key: string;
  /** What the filter box matches against. */
  text: string;
  node: ReactNode;
  description?: string;
};

/**
 * Primer SelectPanel: a titled popover with a filter box and a checkable list.
 * Single mode applies on click and closes. Multi mode collects a draft and
 * applies it when the panel closes, like GitHub's sidebar pickers.
 */
export function SelectPanel({
  trigger,
  title,
  placeholder = "Filter",
  items,
  selected,
  multi,
  onApply,
  onToggle,
  align = "end",
  empty = "No matches",
  loading,
  footer,
}: {
  trigger: ReactElement;
  title: string;
  placeholder?: string;
  items: PanelItem[];
  selected: string[];
  multi?: boolean;
  onApply?: (keys: string[]) => void;
  /** Single mode, several values allowed: called with the clicked key instead of `onApply`. */
  onToggle?: (key: string) => void;
  align?: "start" | "end";
  empty?: string;
  loading?: boolean;
  footer?: ReactNode;
}) {
  const [open, setOpen] = useState(false);
  const [filter, setFilter] = useState("");
  const [draft, setDraft] = useState<string[]>(selected);

  useEffect(() => {
    if (open) {
      setDraft(selected);
      setFilter("");
    }
    // Only when the panel opens: the draft must not be reset while the user picks.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open]);

  function close(next: boolean) {
    if (!next && open && multi) {
      const changed = draft.length !== selected.length || draft.some((k) => !selected.includes(k));
      if (changed) onApply?.(draft);
    }
    setOpen(next);
  }

  function pick(key: string) {
    if (multi) {
      setDraft((d) => (d.includes(key) ? d.filter((k) => k !== key) : [...d, key]));
    } else {
      if (onToggle) onToggle(key);
      else onApply?.(selected.includes(key) ? [] : [key]);
      setOpen(false);
    }
  }

  const needle = filter.trim().toLowerCase();
  const shown = needle ? items.filter((i) => i.text.toLowerCase().includes(needle)) : items;
  const current = multi ? draft : selected;

  return (
    <Popover.Root open={open} onOpenChange={close}>
      <Popover.Trigger render={trigger} />
      <Popover.Portal>
        <Popover.Positioner align={align} sideOffset={4} className="z-50">
          <Popover.Popup className="flex max-h-[min(480px,70vh)] w-[320px] max-w-[calc(100vw-16px)] flex-col rounded-xl bg-popover text-sm shadow-[var(--shadow-floating-small)] outline-none">
            <header className="flex items-center justify-between py-2 pr-2 pl-4">
              <Popover.Title className="font-semibold">{title}</Popover.Title>
              <Popover.Close aria-label="Close" className="flex h-7 w-7 cursor-pointer items-center justify-center rounded-md text-fg-muted hover:bg-control-transparent-hover">
                <XIcon size={16} />
              </Popover.Close>
            </header>
            <div className="border-b border-line px-2 pb-2">
              <input
                autoFocus
                value={filter}
                onChange={(e) => setFilter(e.target.value)}
                placeholder={placeholder}
                aria-label={placeholder}
                className="h-8 w-full rounded-md border border-[var(--control-borderColor-rest)] bg-canvas px-3 outline-none focus:border-fg-accent"
              />
            </div>
            <ul role="listbox" aria-multiselectable={multi} className="min-h-0 flex-1 overflow-y-auto p-2">
              {loading && <li className="px-2 py-4 text-center text-fg-muted">Loading…</li>}
              {!loading && shown.length === 0 && <li className="px-2 py-4 text-center text-fg-muted">{empty}</li>}
              {shown.map((item) => {
                const on = current.includes(item.key);
                return (
                  <li key={item.key} role="option" aria-selected={on}>
                    <button
                      type="button"
                      onClick={() => pick(item.key)}
                      className="flex w-full cursor-pointer items-start gap-2 rounded-md px-2 py-1.5 text-left hover:bg-control-transparent-hover"
                    >
                      <span className={cn("mt-0.5 flex h-4 w-4 shrink-0 items-center justify-center", multi && "rounded-[3px] border", multi && (on ? "border-accent-emphasis bg-accent-emphasis text-fg-on-emphasis" : "border-[var(--control-borderColor-emphasis)]"))}>
                        {on && <CheckIcon size={multi ? 12 : 16} />}
                      </span>
                      <span className="min-w-0 flex-1">
                        <span className="flex items-center gap-2">{item.node}</span>
                        {item.description && <span className="block truncate text-xs text-fg-muted">{item.description}</span>}
                      </span>
                    </button>
                  </li>
                );
              })}
            </ul>
            {footer && <footer className="border-t border-line p-2">{footer}</footer>}
          </Popover.Popup>
        </Popover.Positioner>
      </Popover.Portal>
    </Popover.Root>
  );
}
