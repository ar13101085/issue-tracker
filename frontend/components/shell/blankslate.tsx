import type { ReactNode } from "react";

// Primer "Blankslate": centered empty state.
export function Blankslate({ icon, title, description }: { icon?: ReactNode; title: string; description?: string }) {
  return (
    <div className="flex flex-col items-center px-6 py-8 text-center md:px-10 md:py-20">
      {icon && <div className="mb-2 text-fg-muted">{icon}</div>}
      <h2 className="text-xl font-semibold md:text-2xl">{title}</h2>
      {description && <p className="mt-1 text-fg-muted md:text-base">{description}</p>}
    </div>
  );
}
