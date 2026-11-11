"use client";

import { use } from "react";
import { browser } from "react-dom";
import { LoginPage, SignupPage } from "@/components/pages/auth";
import { IssuePage, NewIssuePage } from "@/components/pages/issue";
import { IssuesListPage } from "@/components/pages/issues-list";
import { LabelsPage } from "@/components/pages/labels";
import { MilestoneFormPage, MilestonesPage } from "@/components/pages/milestones";
import { ProjectPage } from "@/components/pages/project";
import { ProjectsPage } from "@/components/pages/projects";
import { NotificationsPage } from "@/components/pages/notifications";
import { UserSettingsPage } from "@/components/pages/user-settings";
import { HomePage, NewRepoPage, NotFound, OwnerPage, PageError } from "@/components/pages/repos";
import { SettingsPage } from "@/components/pages/settings";
import { AppHeader } from "@/components/shell/app-header";
import { type Route, useRoute } from "@/lib/router";
import { useLive } from "@/lib/live";
import { useRepo, useSession } from "@/lib/session";

/** Repository pages first make sure the repository exists and is visible to the viewer. */
function RepoPage({ route }: { route: Extract<Route, { repo: string }> }) {
  const { error, data } = useRepo(route.owner, route.repo);
  if (error) return <PageError error={error} />;
  if (!data) return null;
  const { owner, repo } = route;
  switch (route.name) {
    case "issues":
      return <IssuesListPage owner={owner} repo={repo} />;
    case "issue-new":
      return <NewIssuePage owner={owner} repo={repo} />;
    case "issue":
      return <IssuePage owner={owner} repo={repo} number={route.number} />;
    case "settings":
      return <SettingsPage owner={owner} repo={repo} section={route.section} />;
    case "labels":
      return <LabelsPage owner={owner} repo={repo} />;
    case "milestones":
      return <MilestonesPage owner={owner} repo={repo} />;
    case "milestone-new":
      return <MilestoneFormPage owner={owner} repo={repo} />;
    case "milestone-edit":
      return <MilestoneFormPage owner={owner} repo={repo} number={route.number} />;
    case "projects":
      return <ProjectsPage owner={owner} repo={repo} />;
    case "project":
      return <ProjectPage owner={owner} repo={repo} number={route.number} />;
  }
}

function Page({ route }: { route: Route }) {
  if ("repo" in route) return <RepoPage route={route} />;
  switch (route.name) {
    case "home":
      return <HomePage />;
    case "owner":
      return <OwnerPage owner={route.owner} />;
    case "new-repo":
      return <NewRepoPage />;
    case "notifications":
      return <NotificationsPage />;
    case "user-settings":
      return <UserSettingsPage section={route.section} />;
    default:
      return <NotFound />;
  }
}

export function App() {
  // The route comes from the address bar, which the prerendered HTML cannot know.
  use(browser("The route is read from the browser location."));
  const route = useRoute();
  const { user } = useSession();
  useLive("repo" in route ? `${route.owner}/${route.repo}` : null, !!user);
  if (route.name === "login") return <LoginPage />;
  if (route.name === "signup") return <SignupPage />;
  return (
    <>
      <AppHeader route={route} />
      <Page route={route} />
    </>
  );
}
