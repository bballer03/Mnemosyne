import "../../test/setup";

import { act, cleanup, render } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it } from "bun:test";
import { MemoryRouter, Navigate, Route, Routes, createMemoryRouter, RouterProvider } from "react-router-dom";

import { routes } from "../../app/router";
import { useArtifactStore } from "../artifact-loader/use-artifact-store";

import { LeakWorkspaceLayout } from "./LeakWorkspaceLayout";
import { useLeakWorkspaceStore } from "./leak-workspace-store";

describe("LeakWorkspaceLayout", () => {
  function seedArtifact() {
    useArtifactStore.setState({
      artifactName: "fixture.json",
      loadError: undefined,
      artifact: {
        summary: {
          heapPath: "fixture.hprof",
          totalObjects: 1,
          totalSizeBytes: 1,
          totalRecords: 1,
        },
        leaks: [
          {
            id: "leak-1",
            className: "com.example.Cache",
            leakKind: "CACHE",
            severity: "HIGH",
            retainedSizeBytes: 100,
            instances: 1,
            description: "Cache leak",
            provenance: [],
          },
        ],
        recommendations: [],
        elapsedSeconds: 1,
        graph: {
          nodeCount: 1,
          edgeCount: 1,
          dominatorCount: 1,
        },
        provenance: [],
      },
    });
  }

  beforeEach(() => {
    act(() => {
      useArtifactStore.getState().reset();
      useLeakWorkspaceStore.getState().reset();
    });
  });

  afterEach(() => {
    cleanup();
    act(() => {
      useArtifactStore.getState().reset();
      useLeakWorkspaceStore.getState().reset();
    });
  });

  it("redirects missing artifacts back to the loader", () => {
    const router = createMemoryRouter(
      [
        { path: "/", element: <div>loader</div> },
        { path: "/leaks/:leakId/*", element: <LeakWorkspaceLayout /> },
      ],
      { initialEntries: ["/leaks/leak-1"] },
    );

    const view = render(<RouterProvider router={router} future={{ v7_startTransition: true }} />);

    expect(view.getByText("loader")).toBeInTheDocument();
  });

  it("renders an invalid-selection state when the leak id is unknown", () => {
    act(() => {
      useArtifactStore.setState({
        artifactName: "fixture.json",
        loadError: undefined,
        artifact: {
          summary: {
            heapPath: "fixture.hprof",
            totalObjects: 1,
            totalSizeBytes: 1,
            totalRecords: 1,
          },
          leaks: [],
          recommendations: [],
          elapsedSeconds: 1,
          graph: {
            nodeCount: 1,
            edgeCount: 1,
            dominatorCount: 1,
          },
          provenance: [],
        },
      });
    });

    const router = createMemoryRouter(
      [
        {
          path: "/leaks/:leakId",
          element: <LeakWorkspaceLayout />,
          children: [{ path: "overview", element: <div>overview placeholder</div> }],
        },
      ],
      { initialEntries: ["/leaks/leak-404/overview"] },
    );

    const view = render(<RouterProvider router={router} future={{ v7_startTransition: true }} />);

    expect(view.getByRole("heading", { name: /leak workspace/i })).toBeInTheDocument();
    expect(view.getByRole("navigation", { name: /leak workspace modes/i })).toBeInTheDocument();
    expect(view.getByRole("link", { name: /overview/i })).toBeInTheDocument();
    expect(view.getByRole("link", { name: /explain/i })).toBeInTheDocument();
    expect(view.getByText(/selected leak was not found in the loaded artifact/i)).toBeInTheDocument();
    expect(view.getAllByRole("link", { name: /back to dashboard/i })).toHaveLength(2);
  });

  it("redirects the leak root route to the overview child route", async () => {
    act(() => {
      useArtifactStore.setState({
        artifactName: "fixture.json",
        loadError: undefined,
        artifact: {
          summary: {
            heapPath: "fixture.hprof",
            totalObjects: 1,
            totalSizeBytes: 1,
            totalRecords: 1,
          },
          leaks: [
            {
              id: "leak-1",
              className: "com.example.Cache",
              leakKind: "CACHE",
              severity: "HIGH",
              retainedSizeBytes: 100,
              instances: 1,
              description: "Cache leak",
              provenance: [],
            },
          ],
          recommendations: [],
          elapsedSeconds: 1,
          graph: {
            nodeCount: 1,
            edgeCount: 1,
            dominatorCount: 1,
          },
          provenance: [],
        },
      });
    });

    const router = createMemoryRouter(
      [
        {
          path: "/leaks/:leakId",
          element: <LeakWorkspaceLayout />,
          children: [
            { index: true, element: <Navigate to="overview" replace /> },
            { path: "overview", element: <div>overview placeholder</div> },
          ],
        },
      ],
      { initialEntries: ["/leaks/leak-1"] },
    );

    const view = render(<RouterProvider router={router} future={{ v7_startTransition: true }} />);

    expect(await view.findByText(/overview placeholder/i)).toBeInTheDocument();
    expect(router.state.location.pathname).toBe("/leaks/leak-1/overview");
  });

  it("uses the real app router to redirect leak root routes into the overview shell", async () => {
    act(() => {
      useArtifactStore.setState({
        artifactName: "fixture.json",
        loadError: undefined,
        artifact: {
          summary: {
            heapPath: "fixture.hprof",
            totalObjects: 1,
            totalSizeBytes: 1,
            totalRecords: 1,
          },
          leaks: [
            {
              id: "leak-1",
              className: "com.example.Cache",
              leakKind: "CACHE",
              severity: "HIGH",
              retainedSizeBytes: 100,
              instances: 1,
              description: "Cache leak",
              provenance: [],
            },
          ],
          recommendations: [],
          elapsedSeconds: 1,
          graph: {
            nodeCount: 1,
            edgeCount: 1,
            dominatorCount: 1,
          },
          provenance: [],
        },
      });
    });

    const router = createMemoryRouter(routes, { initialEntries: ["/leaks/leak-1"] });
    const view = render(<RouterProvider router={router} future={{ v7_startTransition: true }} />);

    expect(await view.findByText(/dependency readiness/i)).toBeInTheDocument();
    expect(view.getByRole("heading", { name: /leak workspace/i })).toBeInTheDocument();
    expect(router.state.location.pathname).toBe("/leaks/leak-1/overview");
  });

  it("redirects cleanly when the loaded artifact disappears after the workspace mounts", () => {
    act(() => {
      useArtifactStore.setState({
        artifactName: "fixture.json",
        loadError: undefined,
        artifact: {
          summary: {
            heapPath: "fixture.hprof",
            totalObjects: 1,
            totalSizeBytes: 1,
            totalRecords: 1,
          },
          leaks: [
            {
              id: "leak-1",
              className: "com.example.Cache",
              leakKind: "CACHE",
              severity: "HIGH",
              retainedSizeBytes: 100,
              instances: 1,
              description: "Cache leak",
              provenance: [],
            },
          ],
          recommendations: [],
          elapsedSeconds: 1,
          graph: {
            nodeCount: 1,
            edgeCount: 1,
            dominatorCount: 1,
          },
          provenance: [],
        },
      });
    });

    const view = render(
      <MemoryRouter initialEntries={["/leaks/leak-1/overview"]}>
        <Routes>
          <Route path="/" element={<div>loader</div>} />
          <Route path="/leaks/:leakId" element={<LeakWorkspaceLayout />}>
            <Route path="overview" element={<div>overview placeholder</div>} />
          </Route>
        </Routes>
      </MemoryRouter>,
    );

    expect(view.getByText(/overview placeholder/i)).toBeInTheDocument();

    act(() => {
      useArtifactStore.getState().reset();
    });

    expect(view.getByText("loader")).toBeInTheDocument();
  });

  it("encodes non-slug-safe leak ids in workspace tab links", () => {
    act(() => {
      useArtifactStore.setState({
        artifactName: "fixture.json",
        loadError: undefined,
        artifact: {
          summary: {
            heapPath: "fixture.hprof",
            totalObjects: 1,
            totalSizeBytes: 1,
            totalRecords: 1,
          },
          leaks: [
            {
              id: "leak id/with spaces?x=1",
              className: "com.example.Cache",
              leakKind: "CACHE",
              severity: "HIGH",
              retainedSizeBytes: 100,
              instances: 1,
              description: "Cache leak",
              provenance: [],
            },
          ],
          recommendations: [],
          elapsedSeconds: 1,
          graph: {
            nodeCount: 1,
            edgeCount: 1,
            dominatorCount: 1,
          },
          provenance: [],
        },
      });
    });

    const router = createMemoryRouter(
      [
        {
          path: "/leaks/:leakId",
          element: <LeakWorkspaceLayout />,
          children: [{ path: "overview", element: <div>overview placeholder</div> }],
        },
      ],
      { initialEntries: ["/leaks/leak id%2Fwith spaces%3Fx=1/overview"] },
    );

    const view = render(<RouterProvider router={router} future={{ v7_startTransition: true }} />);

    expect(view.getByRole("link", { name: /overview/i }).getAttribute("href")).toBe(
      "/leaks/leak%20id%2Fwith%20spaces%3Fx%3D1/overview",
    );
    expect(view.getByRole("link", { name: /explain/i }).getAttribute("href")).toBe(
      "/leaks/leak%20id%2Fwith%20spaces%3Fx%3D1/explain",
    );
  });

  it("renders the workspace shell header and mode navigation for a known leak", () => {
    act(() => {
      useArtifactStore.setState({
        artifactName: "fixture.json",
        loadError: undefined,
        artifact: {
          summary: {
            heapPath: "fixture.hprof",
            totalObjects: 1,
            totalSizeBytes: 1,
            totalRecords: 1,
          },
          leaks: [
            {
              id: "leak-1",
              className: "com.example.Cache",
              leakKind: "CACHE",
              severity: "HIGH",
              retainedSizeBytes: 100,
              instances: 1,
              description: "Cache leak",
              provenance: [],
            },
          ],
          recommendations: [],
          elapsedSeconds: 1,
          graph: {
            nodeCount: 1,
            edgeCount: 1,
            dominatorCount: 1,
          },
          provenance: [],
        },
      });
    });

    const router = createMemoryRouter(
      [
        {
          path: "/leaks/:leakId",
          element: <LeakWorkspaceLayout />,
          children: [{ path: "overview", element: <div>overview placeholder</div> }],
        },
      ],
      { initialEntries: ["/leaks/leak-1/overview"] },
    );

    const view = render(<RouterProvider router={router} future={{ v7_startTransition: true }} />);

    expect(view.getByRole("heading", { name: /leak workspace/i })).toBeInTheDocument();
    expect(view.getByText(/com\.example\.Cache/i)).toBeInTheDocument();
    expect(view.getByRole("navigation", { name: /leak workspace modes/i })).toBeInTheDocument();
    expect(view.getByRole("link", { name: /overview/i })).toHaveAttribute("aria-current", "page");
    expect(view.getByRole("link", { name: /explain/i })).toBeInTheDocument();
    expect(view.getByRole("link", { name: /gc path/i })).toBeInTheDocument();
    expect(view.getByRole("link", { name: /source map/i })).toBeInTheDocument();
    expect(view.getByRole("link", { name: /fix proposal/i })).toBeInTheDocument();
    expect(view.getByText(/overview placeholder/i)).toBeInTheDocument();
  });

  it("seeds leak identity into the workspace store and lets the user apply local context", async () => {
    const user = userEvent.setup();

    act(() => {
      useArtifactStore.setState({
        artifactName: "fixture.json",
        loadError: undefined,
        artifact: {
          summary: {
            heapPath: "fixture.hprof",
            totalObjects: 1,
            totalSizeBytes: 1,
            totalRecords: 1,
          },
          leaks: [
            {
              id: "leak-1",
              className: "com.example.Cache",
              leakKind: "CACHE",
              severity: "HIGH",
              retainedSizeBytes: 100,
              instances: 1,
              description: "Cache leak",
              provenance: [],
            },
          ],
          recommendations: [],
          elapsedSeconds: 1,
          graph: {
            nodeCount: 1,
            edgeCount: 1,
            dominatorCount: 1,
          },
          provenance: [],
        },
      });
    });

    const router = createMemoryRouter(routes, { initialEntries: ["/leaks/leak-1/overview"] });
    const view = render(<RouterProvider router={router} future={{ v7_startTransition: true }} />);

    await view.findByRole("heading", { name: /overview/i });

    expect(useLeakWorkspaceStore.getState().leakId).toBe("leak-1");
    expect(useLeakWorkspaceStore.getState().heapPath).toBe("fixture.hprof");

    await user.clear(view.getByLabelText(/project root/i));
    await user.type(view.getByLabelText(/project root/i), "D:/repo");
    await user.click(view.getByRole("button", { name: /apply project root/i }));

    await user.clear(view.getByLabelText(/object target id/i));
    await user.type(view.getByLabelText(/object target id/i), "0x1234");
    await user.click(view.getByRole("button", { name: /apply object target/i }));

    expect(useLeakWorkspaceStore.getState().projectRoot).toBe("D:/repo");
    expect(useLeakWorkspaceStore.getState().objectId).toBe("0x1234");
  });

  it("activates source-map and gc-path routes after the operator applies local context", async () => {
    const user = userEvent.setup();

    act(() => {
      useArtifactStore.setState({
        artifactName: "fixture.json",
        loadError: undefined,
        artifact: {
          summary: {
            heapPath: "fixture.hprof",
            totalObjects: 1,
            totalSizeBytes: 1,
            totalRecords: 1,
          },
          leaks: [
            {
              id: "leak-1",
              className: "com.example.Cache",
              leakKind: "CACHE",
              severity: "HIGH",
              retainedSizeBytes: 100,
              instances: 1,
              description: "Cache leak",
              provenance: [],
            },
          ],
          recommendations: [],
          elapsedSeconds: 1,
          graph: {
            nodeCount: 1,
            edgeCount: 1,
            dominatorCount: 1,
          },
          provenance: [],
        },
      });
    });

    const router = createMemoryRouter(routes, { initialEntries: ["/leaks/leak-1/overview"] });
    const view = render(<RouterProvider router={router} future={{ v7_startTransition: true }} />);

    await view.findByRole("heading", { name: /overview/i });

    await user.clear(view.getByLabelText(/project root/i));
    await user.type(view.getByLabelText(/project root/i), "D:/repo");
    await user.click(view.getByRole("button", { name: /apply project root/i }));
    await user.click(view.getByRole("link", { name: /source map/i }));

    expect(await view.findByText(/source map unavailable: local source map bridge is unavailable\./i)).toBeInTheDocument();

    await user.clear(view.getByLabelText(/object target id/i));
    await user.type(view.getByLabelText(/object target id/i), "0x1234");
    await user.click(view.getByRole("button", { name: /apply object target/i }));
    await user.click(view.getByRole("link", { name: /gc path/i }));

    expect(await view.findByRole("heading", { name: /gc path/i })).toBeInTheDocument();
    expect(router.state.location.pathname).toBe("/leaks/leak-1/gc-path");
  });

  it("shows recent object targets and reapplies one from the shell", async () => {
    const user = userEvent.setup();

    act(() => {
      seedArtifact();
      useLeakWorkspaceStore.getState().setSelection({
        leakId: "leak-1",
        heapPath: "fixture.hprof",
        objectId: "0x1000",
      });
      useLeakWorkspaceStore.getState().setSelection({ objectId: undefined });
    });

    const router = createMemoryRouter(routes, { initialEntries: ["/leaks/leak-1/overview"] });
    const view = render(<RouterProvider router={router} future={{ v7_startTransition: true }} />);

    expect(await view.findByRole("button", { name: /reuse 0x1000/i })).toBeInTheDocument();

    await user.click(view.getByRole("button", { name: /reuse 0x1000/i }));

    expect(useLeakWorkspaceStore.getState().objectId).toBe("0x1000");
    expect(view.getByLabelText(/object target id/i)).toHaveValue("0x1000");
  });
});
