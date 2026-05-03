// AIProviderConfig, getAIConfig, and updateAIConfig removed in FIX-03.
// Auth flows through AuthState commands. Config management is in Settings.tsx.

import { create } from "zustand";
import type { AIMessage } from "@/types/ai";
import * as commands from "@/lib/tauri-commands";

interface AIState {
  conversations: Map<string, AIMessage[]>; // moduleId -> messages
  isTyping: boolean;

  sendMessage: (trackId: string, moduleId: string | undefined, message: string) => Promise<string>;
  clearConversation: (moduleId: string) => void;
}

export const useAIStore = create<AIState>((set, get) => ({
  conversations: new Map(),
  isTyping: false,

  sendMessage: async (_trackId, moduleId, message) => {
    const key = moduleId ?? "general";
    const state = get();
    const existing = state.conversations.get(key) ?? [];

    const userMsg: AIMessage = { role: "user", content: message };
    const updated = new Map(state.conversations);
    updated.set(key, [...existing, userMsg]);
    set({ conversations: updated, isTyping: true });

    try {
      const response = await commands.sendTutorMessage({
        content: message,
        moduleContext: moduleId,
        history: existing,
      });

      const assistantMsg: AIMessage = { role: "assistant", content: response };
      const final = new Map(get().conversations);
      final.set(key, [...(final.get(key) ?? []), assistantMsg]);
      set({ conversations: final, isTyping: false });

      return response;
    } catch (err) {
      set({ isTyping: false });
      throw err;
    }
  },

  clearConversation: (moduleId) => {
    const updated = new Map(get().conversations);
    updated.delete(moduleId);
    set({ conversations: updated });
  },
}));
