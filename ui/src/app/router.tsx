import { createBrowserRouter, createMemoryRouter, RouterProvider } from "react-router-dom";

import { ArtifactLoaderPage } from "../features/artifact-loader/ArtifactLoaderPage";
import { DashboardPage } from "../features/dashboard/DashboardPage";

const routes = [
  {
    path: "/",
    element: <ArtifactLoaderPage />,
  },
  {
    path: "/dashboard",
    element: <DashboardPage />,
  },
];

const future = {
  v7_startTransition: true,
};

const browserRouter = typeof document === "undefined" ? null : createBrowserRouter(routes);
const memoryRouter = createMemoryRouter(routes);

export function AppRouter() {
  const router = typeof document === "undefined" ? memoryRouter : browserRouter!;

  return <RouterProvider router={router} future={future} />;
}
