import { Routes, Route } from "react-router-dom";
import { AppLayout } from "@/components/layout/AppLayout";
import { Dashboard } from "@/pages/Dashboard";
import { Library } from "@/pages/Library";
import { TrackView } from "@/pages/TrackView";
import { ModuleView } from "@/pages/ModuleView";
import { ReviewSession } from "@/pages/ReviewSession";
import { Settings } from "@/pages/Settings";
import { Onboarding } from "@/pages/Onboarding";
import { DailyChallenge } from "@/pages/DailyChallenge";
import { Achievements } from "@/pages/Achievements";
import { useTheme } from "@/hooks/useTheme";

export default function App() {
  useTheme();

  return (
    <Routes>
      <Route element={<AppLayout />}>
        <Route path="/" element={<Dashboard />} />
        <Route path="/library" element={<Library />} />
        <Route path="/track/:trackId" element={<TrackView />} />
        <Route path="/track/:trackId/module/:moduleId" element={<ModuleView />} />
        <Route path="/review" element={<ReviewSession />} />
        <Route path="/settings" element={<Settings />} />
        <Route path="/daily/today" element={<DailyChallenge />} />
        <Route path="/achievements" element={<Achievements />} />
      </Route>
      <Route path="/onboarding" element={<Onboarding />} />
    </Routes>
  );
}
