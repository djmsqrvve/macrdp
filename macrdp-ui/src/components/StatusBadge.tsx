interface StatusBadgeProps {
  status: "running" | "stopped" | "starting" | "error";
  text?: string;
}

const statusConfig: Record<
  StatusBadgeProps["status"],
  { color: string; label: string }
> = {
  running: { color: "bg-macos-green", label: "Running" },
  stopped: { color: "bg-macos-secondary", label: "Stopped" },
  starting: { color: "bg-macos-yellow", label: "Starting" },
  error: { color: "bg-macos-red", label: "Error" },
};

function StatusBadge({ status, text }: StatusBadgeProps) {
  const config = statusConfig[status];
  return (
    <div className="flex items-center gap-2">
      <div className={`h-2 w-2 rounded-full ${config.color}`} />
      <span className="text-sm text-macos-secondary">
        {text ?? config.label}
      </span>
    </div>
  );
}

export default StatusBadge;
