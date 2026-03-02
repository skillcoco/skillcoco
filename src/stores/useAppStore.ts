import { create } from "zustand";

interface AppState {
  sidebarCollapsed: boolean;
  theme: "dark" | "light";
  isLoading: boolean;
  error: string | null;

  toggleSidebar: () => void;
  setTheme: (theme: "dark" | "light") => void;
  setLoading: (loading: boolean) => void;
  setError: (error: string | null) => void;
}

export const useAppStore = create<AppState>((set) => ({
  sidebarCollapsed: false,
  theme: "dark",
  isLoading: false,
  error: null,

  toggleSidebar: () => set((s) => ({ sidebarCollapsed: !s.sidebarCollapsed })),
  setTheme: (theme) => set({ theme }),
  setLoading: (isLoading) => set({ isLoading }),
  setError: (error) => set({ error }),
}));
