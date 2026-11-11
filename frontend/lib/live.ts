"use client";

// Live updates over server-sent events. The stream only says what changed
// (see crates/server/src/routes/notifications.rs); the matching SWR entries
// are refetched, so open pages update without a reload.

import { useEffect } from "react";
import { useSWRConfig } from "swr";

export function useLive(repo: string | null, signedIn: boolean) {
  const { mutate } = useSWRConfig();
  useEffect(() => {
    if (!repo && !signedIn) return;
    const source = new EventSource(repo ? `/api/stream?repo=${encodeURIComponent(repo)}` : "/api/stream");
    const refetch = (prefix: string) => mutate((key) => typeof key === "string" && key.startsWith(prefix));
    const onIssue = (e: MessageEvent) => {
      const number = JSON.parse(e.data).number;
      refetch(`/repos/${repo}/issues/${number}`);
      refetch(`/repos/${repo}/issues?`);
      refetch(`/repos/${repo}/issues/pinned`);
      refetch(`/repos/${repo}/projects`);
      mutate(`/repos/${repo}`);
    };
    const onRepo = () => refetch(`/repos/${repo}`);
    const onInbox = () => refetch("/notifications");
    source.addEventListener("issue", onIssue);
    source.addEventListener("repo", onRepo);
    source.addEventListener("notifications", onInbox);
    return () => source.close();
  }, [repo, signedIn, mutate]);
}
