import { create } from "zustand";
import type { AIProviderConfig, AIMessage } from "@/types/ai";
import * as commands from "@/lib/tauri-commands";

interface AIState {
  config: AIProviderConfig | null;
  conversations: Map<string, AIMessage[]>; // moduleId -> messages
  isTyping: boolean;

  loadConfig: () => Promise<void>;
  updateConfig: (config: AIProviderConfig) => Promise<void>;
  sendMessage: (trackId: string, moduleId: string | undefined, message: string) => Promise<string>;
  clearConversation: (moduleId: string) => void;
}

export const useAIStore = create<AIState>((set, get) => ({
  config: null,
  conversations: new Map(),
  isTyping: false,

  loadConfig: async () => {
    try {
      const config = await commands.getAIConfig();
      set({ config });
    } catch (err) {
      console.error("Failed to load AI config:", err);
    }
  },

  updateConfig: async (config) => {
    await commands.updateAIConfig(config);
    set({ config });
  },

  sendMessage: async (trackId, moduleId, message) => {
    const key = moduleId ?? "general";
    const state = get();
    const existing = state.conversations.get(key) ?? [];

    const userMsg: AIMessage = { role: "user", content: message };
    const updated = new Map(state.conversations);
    updated.set(key, [...existing, userMsg]);
    set({ conversations: updated, isTyping: true });

    try {
      const response = await commands.sendTutorMessage({
        message,
        context: {
          trackId,
          moduleId,
          learnerHistory: JSON.stringify(existing.slice(-10)), // last 10 messages for context
        },
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
