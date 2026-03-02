import { Flame, Clock, Brain } from "lucide-react";
import { useLearningStore } from "@/stores/useLearningStore";

export function BottomBar() {
  const dueCards = useLearningStore((s) => s.dueCards);

  return (
    <div className="flex h-10 items-center justify-between border-t border-border bg-card px-6 text-xs text-muted-foreground">
      <div className="flex items-center gap-4">
        <span className="flex items-center gap-1.5">
          <Flame size={14} className="text-orange-500" />
          <span>0 day streak</span>
        </span>
        <span className="flex items-center gap-1.5">
          <Brain size={14} />
          <span>{dueCards.length} cards due</span>
        </span>
      </div>
      <div className="flex items-center gap-1.5">
        <Clock size={14} />
        <span>0m today</span>
      </div>
    </div>
  );
}
