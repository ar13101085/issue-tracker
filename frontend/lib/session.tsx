"use client";

import useSWR from "swr";
import { api, ApiError, type CurrentUser, type Repo } from "@/lib/api";

/** The signed-in account: `undefined` while loading, `null` when signed out. */
export function useSession() {
  const { data, mutate } = useSWR<CurrentUser | null>("/user", async (path: string) => {
    try {
      return await api<CurrentUser>(path);
    } catch (e) {
      if (e instanceof ApiError && e.status === 401) return null;
      throw e;
    }
  });
  return { user: data, setUser: (u: CurrentUser | null) => mutate(u, { revalidate: false }) };
}

export function useRepo(owner: string | undefined, repo: string | undefined) {
  return useSWR<Repo, ApiError>(owner && repo ? `/repos/${owner}/${repo}` : null, api);
}
