import { NavLink } from "react-router-dom";
import {
  LayoutDashboard,
  RotateCcw,
  Library,
  Settings,
  Sun,
  Moon,
  ChevronLeft,
} from "lucide-react";
import { useAppStore } from "@/stores/useAppStore";
import { useLearningStore } from "@/stores/useLearningStore";
import { useTheme } from "@/hooks/useTheme";
import { cn } from "@/lib/utils";
import { getTrackColor } from "@/lib/track-colors";

export function Sidebar() {
  const collapsed = useAppStore((s) => s.sidebarCollapsed);
  const toggleSidebar = useAppStore((s) => s.toggleSidebar);
  const tracks = useLearningStore((s) => s.tracks);
  const dueCards = useLearningStore((s) => s.dueCards);
  const { theme, toggleTheme } = useTheme();

  const dueCount = dueCards.length;

  return (
    <aside
      className={cn(
        "fixed left-0 top-0 z-40 flex h-screen flex-col transition-all duration-200",
        "border-r border-white/10",
        "bg-background/60 backdrop-blur-xl backdrop-saturate-150",
        "supports-[backdrop-filter]:bg-background/60",
        collapsed ? "w-16" : "w-64"
      )}
    >
      {/* Logo Area */}
      <div className="flex h-14 items-center justify-between px-4">
        {!collapsed && (
          <div className="flex items-center gap-2">
            <img src="/coco.svg" alt="" aria-hidden="true" className="h-7 w-7" />
            <span className="text-lg font-bold text-foreground">
              Skill<span className="text-primary">Coco</span>
            </span>
          </div>
        )}
        <button
          onClick={toggleSidebar}
          className={cn(
            "rounded-md p-1.5 text-muted-foreground transition-colors",
            "hover:bg-white/10 hover:text-foreground",
            collapsed && "mx-auto"
          )}
          aria-label={collapsed ? "Expand sidebar" : "Collapse sidebar"}
        >
          <ChevronLeft
            size={18}
            className={cn(
              "transition-transform duration-200",
              collapsed && "rotate-180"
            )}
          />
        </button>
      </div>

      {/* Navigation Section */}
      <nav className="flex-1 overflow-y-auto px-2 pt-4">
        {!collapsed && (
          <span className="mb-2 block px-3 text-[10px] font-semibold uppercase tracking-widest text-muted-foreground/70">
            Navigation
          </span>
        )}

        <NavLink
          to="/"
          end
          className={({ isActive }) =>
            cn(
              "group relative flex items-center gap-3 rounded-md px-3 py-2 text-sm transition-colors",
              isActive
                ? "bg-white/10 font-medium text-foreground"
                : "text-muted-foreground hover:bg-white/5 hover:text-foreground",
              collapsed && "justify-center"
            )
          }
        >
          {({ isActive }) => (
            <>
              {isActive && (
                <span className="absolute left-0 top-1/2 h-5 w-[3px] -translate-y-1/2 rounded-r-sm bg-primary" />
              )}
              <LayoutDashboard size={18} />
              {!collapsed && <span>Dashboard</span>}
            </>
          )}
        </NavLink>

        <NavLink
          to="/review"
          className={({ isActive }) =>
            cn(
              "group relative flex items-center gap-3 rounded-md px-3 py-2 text-sm transition-colors",
              isActive
                ? "bg-white/10 font-medium text-foreground"
                : "text-muted-foreground hover:bg-white/5 hover:text-foreground",
              collapsed && "justify-center"
            )
          }
        >
          {({ isActive }) => (
            <>
              {isActive && (
                <span className="absolute left-0 top-1/2 h-5 w-[3px] -translate-y-1/2 rounded-r-sm bg-primary" />
              )}
              <RotateCcw size={18} />
              {!collapsed && (
                <span className="flex-1">
                  Review
                  {dueCount > 0 && (
                    <span className="ml-2 inline-flex items-center rounded-full bg-primary/20 px-1.5 py-0.5 text-[10px] font-medium text-primary">
                      {dueCount} due
                    </span>
                  )}
                </span>
              )}
              {collapsed && dueCount > 0 && (
                <span className="absolute -right-0.5 -top-0.5 flex h-4 w-4 items-center justify-center rounded-full bg-accent text-[9px] font-bold text-accent-foreground">
                  {dueCount > 99 ? "99" : dueCount}
                </span>
              )}
            </>
          )}
        </NavLink>

        <NavLink
          to="/library"
          className={({ isActive }) =>
            cn(
              "group relative flex items-center gap-3 rounded-md px-3 py-2 text-sm transition-colors",
              isActive
                ? "bg-white/10 font-medium text-foreground"
                : "text-muted-foreground hover:bg-white/5 hover:text-foreground",
              collapsed && "justify-center"
            )
          }
        >
          {({ isActive }) => (
            <>
              {isActive && (
                <span className="absolute left-0 top-1/2 h-5 w-[3px] -translate-y-1/2 rounded-r-sm bg-primary" />
              )}
              <Library size={18} />
              {!collapsed && <span>Library</span>}
            </>
          )}
        </NavLink>

        {/* Learning Tracks Section */}
        {!collapsed && (
          <div className="mt-6">
            <div className="mb-2 flex items-center justify-between px-3">
              <span className="text-[10px] font-semibold uppercase tracking-widest text-muted-foreground/70">
                Learning Tracks
              </span>
            </div>

            <div className="space-y-0.5">
              {tracks.map((track) => {
                const color = getTrackColor(track.topic);
                return (
                  <NavLink
                    key={track.id}
                    to={`/track/${track.id}`}
                    className={({ isActive }) =>
                      cn(
                        "group relative flex items-center gap-3 rounded-md px-3 py-2 text-sm transition-colors",
                        isActive
                          ? "bg-white/10 font-medium text-foreground"
                          : "text-muted-foreground hover:bg-white/5 hover:text-foreground"
                      )
                    }
                  >
                    {({ isActive }) => (
                      <>
                        {isActive && (
                          <span
                            className="absolute left-0 top-1/2 h-5 w-[3px] -translate-y-1/2 rounded-r-sm"
                            style={{ backgroundColor: color }}
                          />
                        )}
                        <span
                          className="h-2 w-2 shrink-0 rounded-full"
                          style={{ backgroundColor: color }}
                        />
                        <div className="flex flex-1 items-center gap-2 overflow-hidden">
                          <span className="truncate">{track.topic}</span>
                          <div className="ml-auto flex shrink-0 items-center gap-1.5">
                            <span className="text-[11px] tabular-nums text-muted-foreground">
                              {Math.round(track.progressPercent)}%
                            </span>
                            <div className="h-1 w-8 overflow-hidden rounded-full bg-white/10">
                              <div
                                className="h-full rounded-full transition-all duration-300"
                                style={{
                                  width: `${Math.round(track.progressPercent)}%`,
                                  backgroundColor: color,
                                }}
                              />
                            </div>
                          </div>
                        </div>
                      </>
                    )}
                  </NavLink>
                );
              })}
            </div>
          </div>
        )}

        {/* Collapsed state: show track dots */}
        {collapsed && tracks.length > 0 && (
          <div className="mt-6 flex flex-col items-center gap-2">
            {tracks.map((track) => (
              <NavLink
                key={track.id}
                to={`/track/${track.id}`}
                className="rounded-md p-1.5 transition-colors hover:bg-white/10"
                title={`${track.topic} (${Math.round(track.progressPercent)}%)`}
              >
                <span
                  className="block h-2.5 w-2.5 rounded-full"
                  style={{ backgroundColor: getTrackColor(track.topic) }}
                />
              </NavLink>
            ))}
          </div>
        )}
      </nav>

      {/* Bottom Section: Settings + Theme Toggle */}
      <div className="border-t border-white/10 p-2">
        <NavLink
          to="/settings"
          className={({ isActive }) =>
            cn(
              "group relative flex items-center gap-3 rounded-md px-3 py-2 text-sm transition-colors",
              isActive
                ? "bg-white/10 font-medium text-foreground"
                : "text-muted-foreground hover:bg-white/5 hover:text-foreground",
              collapsed && "justify-center"
            )
          }
        >
          {({ isActive }) => (
            <>
              {isActive && (
                <span className="absolute left-0 top-1/2 h-5 w-[3px] -translate-y-1/2 rounded-r-sm bg-primary" />
              )}
              <Settings size={18} />
              {!collapsed && <span>Settings</span>}
            </>
          )}
        </NavLink>

        <button
          onClick={toggleTheme}
          className={cn(
            "flex w-full items-center gap-3 rounded-md px-3 py-2 text-sm transition-colors",
            "text-muted-foreground hover:bg-white/5 hover:text-foreground",
            collapsed && "justify-center"
          )}
          aria-label={`Switch to ${theme === "dark" ? "light" : "dark"} mode`}
        >
          {theme === "dark" ? <Sun size={18} /> : <Moon size={18} />}
          {!collapsed && (
            <span>{theme === "dark" ? "Light Mode" : "Dark Mode"}</span>
          )}
        </button>
      </div>
    </aside>
  );
}
