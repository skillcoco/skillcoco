import { Outlet } from "react-router-dom";
import { Sidebar } from "./Sidebar";
import { BottomBar } from "./BottomBar";
import { useAppStore } from "@/stores/useAppStore";
import { cn } from "@/lib/utils";

export function AppLayout() {
  const sidebarCollapsed = useAppStore((s) => s.sidebarCollapsed);

  return (
    <div className="flex h-screen overflow-hidden bg-background/95">
      <Sidebar />
      <div
        className={cn(
          // The sidebar is position:fixed (zero flex space), so a flex-1
          // content pane resolves to the full viewport width and ml-64 then
          // pushes it past the right edge → the right strip got clipped by
          // the root's overflow-hidden. Constrain width to viewport − sidebar
          // so margin + width == 100% exactly.
          "flex flex-col transition-all duration-200",
          sidebarCollapsed
            ? "ml-16 w-[calc(100%-4rem)]"
            : "ml-64 w-[calc(100%-16rem)]"
        )}
      >
        <main className="flex-1 overflow-y-auto p-6">
          <Outlet />
        </main>
        <BottomBar />
      </div>
    </div>
  );
}
