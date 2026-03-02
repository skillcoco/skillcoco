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
          "flex flex-1 flex-col transition-all duration-200",
          sidebarCollapsed ? "ml-16" : "ml-64"
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
