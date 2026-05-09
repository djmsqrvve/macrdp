import { Card } from "./ui/card";
import type { Metrics } from "../lib/types";

interface MetricsStripProps {
  metrics: Metrics;
  port: number;
}

interface MetricItemProps {
  label: string;
  value: string;
  unit?: string;
  sub?: string;
  children?: React.ReactNode;
}

function MetricItem({ label, value, unit, sub, children }: MetricItemProps) {
  return (
    <div className="flex-1 px-4 py-3.5 relative min-w-0
      [&:not(:last-child)]:after:content-[''] [&:not(:last-child)]:after:absolute
      [&:not(:last-child)]:after:right-0 [&:not(:last-child)]:after:top-3
      [&:not(:last-child)]:after:bottom-3 [&:not(:last-child)]:after:w-px
      [&:not(:last-child)]:after:bg-border">
      <div className="text-[11px] font-medium uppercase tracking-wide text-muted-foreground mb-1 whitespace-nowrap">
        {label}
      </div>
      <div className="text-[22px] font-bold tracking-tight leading-none whitespace-nowrap">
        {value}
        {unit && <span className="text-xs font-normal text-muted-foreground ml-0.5">{unit}</span>}
      </div>
      {sub && <div className="text-[10px] text-muted-foreground mt-0.5 whitespace-nowrap">{sub}</div>}
      {children}
    </div>
  );
}

function QualityBar({ quality }: { quality: number }) {
  const segments = 5;
  const active = Math.round(quality * segments);
  return (
    <div className="flex gap-0.5 mt-1.5">
      {Array.from({ length: segments }, (_, i) => (
        <div
          key={i}
          className={`w-4 h-1 rounded-sm ${i < active ? "bg-green-500" : "bg-border"}`}
        />
      ))}
    </div>
  );
}

function formatBytes(bytes: number): [string, string] {
  if (bytes >= 1e9) return [(bytes / 1e9).toFixed(1), "GB"];
  if (bytes >= 1e6) return [(bytes / 1e6).toFixed(1), "MB"];
  if (bytes >= 1e3) return [(bytes / 1e3).toFixed(0), "KB"];
  return [String(bytes), "B"];
}

const qualityLabel = (q: number) =>
  q >= 0.8 ? "Excellent" : q >= 0.6 ? "Good" : q >= 0.4 ? "Fair" : q >= 0.2 ? "Poor" : "Very Poor";

export default function MetricsStrip({ metrics, port }: MetricsStripProps) {
  const [bytesVal, bytesUnit] = formatBytes(metrics.bytes_sent);
  return (
    <Card className="flex items-stretch overflow-x-auto">
      <MetricItem label="Port" value={String(port)} sub="0.0.0.0" />
      <MetricItem label="Frame Rate" value={String(metrics.fps)} unit="FPS" />
      <MetricItem label="Bitrate" value={(metrics.bitrate_kbps / 1000).toFixed(1)} unit="Mbps" />
      <MetricItem label="RTT" value={metrics.latency_ms.toFixed(1)} unit="ms" sub="EWMA" />
      <MetricItem label="Encode" value={metrics.encode_ms.toFixed(1)} unit="ms" />
      <MetricItem label="Network" value={metrics.net_ms.toFixed(1)} unit="ms" />
      <MetricItem label="Frame Size" value={String(Math.round(metrics.last_frame_bytes / 1024))} unit="KB" />
      <MetricItem label="Total Traffic" value={bytesVal} unit={bytesUnit} />
      <MetricItem label="Network Quality" value={qualityLabel(metrics.network_quality)}>
        <QualityBar quality={metrics.network_quality} />
      </MetricItem>
    </Card>
  );
}
