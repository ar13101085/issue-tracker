import type { InputHTMLAttributes, ReactNode, TextareaHTMLAttributes } from "react";
import { AlertIcon, XIcon } from "@primer/octicons-react";
import { cn } from "@/lib/utils";

// Primer TextInput.
export const inputClass =
  "h-8 w-full rounded-md border border-[var(--control-borderColor-rest)] bg-canvas px-3 text-sm text-fg shadow-[var(--shadow-inset)] placeholder:text-fg-muted focus:border-fg-accent focus:outline focus:outline-2 focus:-outline-offset-1 focus:outline-[var(--focus-outlineColor)] disabled:bg-[var(--bgColor-disabled)] aria-invalid:border-[var(--borderColor-danger-emphasis)]";

export function TextInput({ className, ...rest }: InputHTMLAttributes<HTMLInputElement>) {
  return <input className={cn(inputClass, className)} {...rest} />;
}

export function Textarea({ className, ...rest }: TextareaHTMLAttributes<HTMLTextAreaElement>) {
  return <textarea className={cn(inputClass, "h-auto min-h-[100px] py-2 leading-normal", className)} {...rest} />;
}

/** Label, control, and the caption or error under it. */
export function Field({
  label,
  htmlFor,
  required,
  error,
  caption,
  children,
}: {
  label: string;
  htmlFor: string;
  required?: boolean;
  error?: string;
  caption?: ReactNode;
  children: ReactNode;
}) {
  return (
    <div className="flex flex-col gap-1">
      <label htmlFor={htmlFor} className="text-sm font-semibold">
        {label}
        {required && <span className="ml-1 text-fg-danger">*</span>}
      </label>
      {children}
      {error ? (
        <p className="flex items-center gap-1 text-xs font-semibold text-fg-danger">
          <AlertIcon size={12} />
          {error}
        </p>
      ) : (
        caption && <p className="text-xs text-fg-muted">{caption}</p>
      )}
    </div>
  );
}

/** Primer Flash banner. */
export function Flash({
  variant = "danger",
  onDismiss,
  children,
  className,
}: {
  variant?: "danger" | "success" | "warning" | "info";
  onDismiss?: () => void;
  children: ReactNode;
  className?: string;
}) {
  const colors = {
    danger: "border-[var(--borderColor-danger-muted)] bg-[var(--bgColor-danger-muted)]",
    success: "border-[var(--borderColor-success-muted)] bg-[var(--bgColor-success-muted)]",
    warning: "border-[var(--borderColor-attention-muted)] bg-[var(--bgColor-attention-muted)]",
    info: "border-[var(--borderColor-accent-muted)] bg-[var(--bgColor-accent-muted)]",
  }[variant];
  return (
    <div role="alert" className={cn("flex items-start gap-2 rounded-md border px-4 py-3 text-sm", colors, className)}>
      <div className="flex-1">{children}</div>
      {onDismiss && (
        <button type="button" aria-label="Dismiss" onClick={onDismiss} className="text-fg-muted hover:text-fg">
          <XIcon size={16} />
        </button>
      )}
    </div>
  );
}
