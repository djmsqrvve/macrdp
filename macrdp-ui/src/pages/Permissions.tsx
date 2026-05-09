import { useState, useEffect } from "react";
import { useLocation, useNavigate } from "react-router-dom";
import { Info } from "lucide-react";
import { api } from "../lib/ipc";
import type { PermissionStatus } from "../lib/types";
import { Button } from "@/components/ui/button";
import { Alert } from "@/components/ui/alert";
import PermissionCard from "../components/PermissionCard";

const permissionDefs = [
  {
    key: "screen_capture" as keyof PermissionStatus,
    name: "Screen Recording",
    description: "Required to capture screen content",
    pane: "screen_capture",
    iconKey: "screen_capture" as const,
  },
  {
    key: "accessibility" as keyof PermissionStatus,
    name: "Accessibility",
    description: "Required to inject keyboard and mouse events",
    pane: "accessibility",
    iconKey: "accessibility" as const,
  },
  {
    key: "microphone" as keyof PermissionStatus,
    name: "Microphone",
    description: "Required for audio forwarding (Phase 3)",
    pane: "microphone",
    iconKey: "microphone" as const,
  },
];

function Permissions() {
  const [perms, setPerms] = useState<PermissionStatus | null>(null);
  const location = useLocation();
  const navigate = useNavigate();
  const firstLaunch = (location.state as { firstLaunch?: boolean })?.firstLaunch ?? false;

  useEffect(() => {
    // Initial fetch
    api.getPermissions().then(setPerms).catch(console.error);

    // Poll every 5 seconds
    const interval = setInterval(() => {
      api.getPermissions().then(setPerms).catch(console.error);
    }, 5000);

    return () => clearInterval(interval);
  }, []);

  const allRequiredGranted =
    perms?.screen_capture === true && perms?.accessibility === true;

  return (
    <div className="space-y-6">
      {firstLaunch && (
        <Alert>
          <Info className="h-4 w-4" />
          <span className="text-sm font-medium">
            First-time use requires granting the following permissions
          </span>
        </Alert>
      )}

      <h1 className="text-lg font-semibold text-foreground">
        System Permissions
      </h1>

      <div className="space-y-3">
        {permissionDefs.map((def) => (
          <PermissionCard
            key={def.key}
            name={def.name}
            description={def.description}
            granted={perms?.[def.key] ?? false}
            pane={def.pane}
            iconKey={def.iconKey}
          />
        ))}
      </div>

      {firstLaunch && (
        <div className="flex gap-3">
          <Button
            disabled={!allRequiredGranted}
            onClick={() => navigate("/")}
          >
            Continue
          </Button>
          <Button
            variant="outline"
            onClick={() => navigate("/")}
          >
            Skip
          </Button>
        </div>
      )}
    </div>
  );
}

export default Permissions;
