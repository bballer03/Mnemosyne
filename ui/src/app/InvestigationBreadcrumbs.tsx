import { Link, useLocation } from "react-router-dom";

function labelForPath(pathname: string): string {
  if (pathname.startsWith("/artifacts/explorer")) {
    return "Histogram / Explorer";
  }
  if (pathname.includes("/object-inspector")) {
    return "Object Inspector";
  }
  if (pathname.includes("/dominators")) {
    return "Dominators";
  }
  if (pathname.includes("/query-console")) {
    return "Query Console";
  }
  if (pathname.includes("/threads")) {
    return "Threads";
  }
  if (pathname.includes("/gc-path")) {
    return "GC Path";
  }
  if (pathname.includes("/leaks/")) {
    return "Leak Workspace";
  }
  if (pathname === "/dashboard") {
    return "Dashboard";
  }
  if (pathname === "/") {
    return "Home";
  }
  return pathname;
}

/**
 * URL-driven investigation crumbs. Does not hold a second heap graph —
 * only reflects the current location plus optional objectId query seed.
 */
export function InvestigationBreadcrumbs() {
  const location = useLocation();
  const objectId = new URLSearchParams(location.search).get("objectId");
  const crumbs = [
    { to: "/", label: "Home" },
    { to: location.pathname + location.search, label: labelForPath(location.pathname) },
  ];

  if (objectId && !location.pathname.includes("/object-inspector")) {
    crumbs.push({
      to: `/heap-explorer/object-inspector?objectId=${encodeURIComponent(objectId)}`,
      label: `Object ${objectId}`,
    });
  }

  return (
    <nav
      aria-label="Investigation"
      style={{ display: "flex", flexWrap: "wrap", gap: "0.35rem", alignItems: "center" }}
    >
      {crumbs.map((crumb, index) => (
        <span
          key={`${crumb.to}-${index}`}
          style={{ display: "inline-flex", gap: "0.35rem", alignItems: "center" }}
        >
          {index > 0 ? <span style={{ color: "#64748b" }}>/</span> : null}
          <Link to={crumb.to} style={{ color: index === crumbs.length - 1 ? "#e2e8f0" : "#94a3b8" }}>
            {crumb.label}
          </Link>
        </span>
      ))}
    </nav>
  );
}
