import { Suspense } from "react";
import { App } from "@/components/app";
import { HeaderSkeleton } from "@/components/shell/app-header";

// The only page. The Rust server answers every app URL with this HTML, so the
// prerendered part must not depend on the path: it is just the header skeleton.
export default function Page() {
  return (
    <Suspense fallback={<HeaderSkeleton />}>
      <App />
    </Suspense>
  );
}
