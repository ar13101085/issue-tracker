"use client";

import { Dialog as Base } from "@base-ui/react/dialog";
import { XIcon } from "@primer/octicons-react";
import type { ReactNode } from "react";

/** Primer Dialog: centered on desktop, a bottom sheet on phones. */
export function Dialog({
  open,
  onClose,
  title,
  subtitle,
  width = 480,
  footer,
  children,
}: {
  open: boolean;
  onClose: () => void;
  title: string;
  subtitle?: ReactNode;
  width?: number;
  footer?: ReactNode;
  children: ReactNode;
}) {
  return (
    <Base.Root open={open} onOpenChange={(next) => !next && onClose()}>
      <Base.Portal>
        <Base.Backdrop className="fixed inset-0 z-50 bg-[var(--overlay-backdrop-bgColor)]" />
        <Base.Popup
          style={{ maxWidth: width }}
          className="fixed inset-x-0 bottom-0 z-50 mx-auto flex max-h-[85vh] w-full flex-col rounded-t-xl bg-popover shadow-[var(--shadow-floating-large)] outline-none sm:top-1/2 sm:bottom-auto sm:w-[calc(100%-32px)] sm:-translate-y-1/2 sm:rounded-xl"
        >
          <header className="flex items-start gap-2 border-b border-line p-2 pl-4">
            <div className="min-w-0 flex-1 py-1.5">
              <Base.Title className="text-sm font-semibold">{title}</Base.Title>
              {subtitle && <div className="text-xs text-fg-muted">{subtitle}</div>}
            </div>
            <Base.Close aria-label="Close" className="flex h-8 w-8 cursor-pointer items-center justify-center rounded-md text-fg-muted hover:bg-control-transparent-hover">
              <XIcon size={16} />
            </Base.Close>
          </header>
          <div className="min-h-0 flex-1 overflow-y-auto p-4">{children}</div>
          {footer && <footer className="flex flex-wrap justify-end gap-2 border-t border-line p-4">{footer}</footer>}
        </Base.Popup>
      </Base.Portal>
    </Base.Root>
  );
}
