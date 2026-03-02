import { Link, useLocation } from "react-router-dom";
import {
  LayoutDashboard,
  BookOpen,
  Brain,
  Settings,
  PanelLeftClose,
  PanelLeft,
  Plus,
} from "lucide-react";
import { useAppStore } from "@/stores/useAppStore";
import { useLearningStore } from "@/stores/useLearningStore";
import { cn } from "@/lib/utils";

const navItems = [
  { icon: LayoutDashboard, label: "Dashboard", path: "/" },
  { icon: Brain, label: "Review", path: "/review" },
  { icon: Settings, label: "Settings", path: "/settings" },
];

export function Sidebar() {
  const location = useLocation();
  const collapsed = useAppStore((s) => s.sidebarCollapsed);
  const toggleSidebar = useAppStore((s) => s.toggleSidebar);
  const tracks = useLearningStore((s) => s.tracks);

  return (
    <aside
      className={cn(
        "fixed left-0 top-0 z-40 flex h-screen flex-col border-r border-border bg-sidebar transition-all duration-200",
        collapsed ? "w-16" : "w-64"
      )}
    >
      {/* Logo */}
      <div className="flex h-14 items-center justify-between border-b border-border px-4">
        {!collapsed && (
          <span className="text-lg font-bold text-foreground">
            Learn<span className="text-primary">Forge</span>
          </span>
        )}
        <button
          onClick={toggleSidebar}
          className="rounded-md p-1.5 text-muted-foreground hover:bg-sidebar-accent hover:text-foreground"
        >
          {collapsed ? <PanelLeft size={18} /> : <PanelLeftClose size={18} />}
        </button>
      </div>

      {/* Navigation */}
      <nav className="flex-1 space-y-1 p-2">
        {navItems.map((item) => {
          const active = location.pathname === item.path;
          return (
            <Link
              key={item.path}
              to={item.path}
              className={cn(
                "flex items-center gap-3 rounded-md px-3 py-2 text-sm transition-colors",
                active
                  ? "bg-sidebar-accent text-foreground font-medium"
                  : "text-muted-foreground hover:bg-sidebar-accent hover:text-foreground"
              )}
            >
              <item.icon size={18} />
              {!collapsed && <span>{item.label}</span>}
            </Link>
          );
        })}

        {/* Active Tracks */}
        {!collapsed && (
          <div className="mt-6">
            <div className="flex items-center justify-between px-3 py-2">
              <span className="text-xs font-semibold uppercase tracking-wider text-muted-foreground">
                Tracks
              </span>
              <Link
                to="/onboarding"
                className="rounded-md p-1 text-muted-foreground hover:bg-sidebar-accent hover:text-foreground"
              >
                <Plus size={14} />
              </Link>
            </div>
            {tracks.map((track) => (
              <Link
                key={track.id}
                to={`/track/${track.id}`}
                className={cn(
                  "flex items-center gap-3 rounded-md px-3 py-2 text-sm transition-colors",
                  location.pathname.includes(track.id)
                    ? "bg-sidebar-accent text-foreground font-medium"
                    : "text-muted-foreground hover:bg-sidebar-accent hover:text-foreground"
                )}
              >
                <BookOpen size={16} />
                <div className="flex-1 truncate">
                  <div className="truncate">{track.topic}</div>
                  <div className="text-xs text-muted-foreground">
                    {track.progressPercent}% complete
                  </div>
                </div>
              </Link>
            ))}
          </div>
        )}
      </nav>
    </aside>
  );
}
