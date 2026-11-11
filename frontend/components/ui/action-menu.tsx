"use client";

import { Menu } from "@base-ui/react/menu";
import { CheckIcon } from "@primer/octicons-react";
import type { ReactElement, ReactNode } from "react";
import { cn } from "@/lib/utils";

// Primer ActionMenu on top of Base UI's Menu.
export function ActionMenu({
  trigger,
  align = "end",
  width = 192,
  children,
}: {
  trigger: ReactElement;
  align?: "start" | "end";
  width?: number;
  children: ReactNode;
}) {
  return (
    <Menu.Root>
      <Menu.Trigger render={trigger} />
      <Menu.Portal>
        <Menu.Positioner align={align} sideOffset={4} className="z-50">
          <Menu.Popup
            style={{ minWidth: width }}
            className="max-w-[calc(100vw-16px)] rounded-xl bg-popover p-2 text-sm shadow-[var(--shadow-floating-small)] outline-none"
          >
            {children}
          </Menu.Popup>
        </Menu.Positioner>
      </Menu.Portal>
    </Menu.Root>
  );
}

const itemClass =
  "flex min-h-8 cursor-pointer items-center gap-2 rounded-md px-2 py-1.5 outline-none select-none data-[highlighted]:bg-control-transparent-hover";

export function ActionItem({
  onSelect,
  danger,
  leading,
  children,
}: {
  onSelect?: () => void;
  danger?: boolean;
  leading?: ReactNode;
  children: ReactNode;
}) {
  return (
    <Menu.Item onClick={onSelect} className={cn(itemClass, danger && "text-fg-danger")}>
      {leading && <span className={danger ? "" : "text-fg-muted"}>{leading}</span>}
      {children}
    </Menu.Item>
  );
}

/** Single-select item: a check mark in front of the chosen one. */
export function ActionChoice({ selected, onSelect, children }: { selected: boolean; onSelect: () => void; children: ReactNode }) {
  return (
    <Menu.Item onClick={onSelect} role="menuitemradio" aria-checked={selected} className={itemClass}>
      <span className="flex w-4 justify-center">{selected && <CheckIcon size={16} />}</span>
      {children}
    </Menu.Item>
  );
}

export function ActionDivider() {
  return <Menu.Separator className="-mx-2 my-2 h-px bg-line-muted" />;
}

export function ActionHeading({ children }: { children: ReactNode }) {
  return <div className="px-2 py-1.5 text-xs font-semibold text-fg-muted">{children}</div>;
}
