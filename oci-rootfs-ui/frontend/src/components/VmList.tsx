import { useQuery } from "@tanstack/react-query";
import axios from "axios";

interface VmInfo {
  name: string;
  image: string;
  layers: string[];
  layers_downloaded: number;
  layers_cached: number;
  bytes_downloaded: number;
  bytes_total: number;
  pull_duration_ms: number;
  pulled_at: string;
  socket_active: boolean;
  upper_size: number;
}

function formatBytes(b: number) {
  if (b === 0) return "0 B";
  if (b < 1024) return `${b} B`;
  if (b < 1024 * 1024) return `${(b / 1024).toFixed(1)} KB`;
  return `${(b / 1024 / 1024).toFixed(1)} MB`;
}

function formatDuration(ms: number) {
  if (ms === 0) return "—";
  if (ms < 1000) return `${ms}ms`;
  return `${(ms / 1000).toFixed(2)}s`;
}

function formatDate(iso: string) {
  if (!iso) return "—";
  return new Date(iso).toLocaleString("fr-FR", {
    day: "2-digit",
    month: "2-digit",
    year: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}

export function VmList() {
  const { data: vms = [], isLoading } = useQuery({
    queryKey: ["vms"],
    queryFn: () => axios.get<VmInfo[]>("/api/vms").then((r) => r.data),
  });

  return (
    <section style={{ marginBottom: "2rem" }}>
      <h2
        style={{
          fontSize: 13,
          fontWeight: 500,
          color: "#888",
          textTransform: "uppercase",
          letterSpacing: "0.05em",
          marginBottom: 10,
        }}
      >
        VMs — {vms.length}
      </h2>

      {isLoading && (
        <p style={{ fontSize: 13, color: "#888" }}>chargement...</p>
      )}

      {vms.map((vm) => (
        <div
          key={vm.name}
          style={{
            background: "#fff",
            border: "0.5px solid #e5e5e5",
            borderRadius: 12,
            padding: "1rem 1.25rem",
            marginBottom: 10,
          }}
        >
          {/* Header */}
          <div
            style={{
              display: "flex",
              justifyContent: "space-between",
              alignItems: "center",
              marginBottom: 12,
            }}
          >
            <div>
              <span
                style={{
                  fontFamily: "monospace",
                  fontSize: 15,
                  fontWeight: 500,
                }}
              >
                {vm.name}
              </span>
              <span
                style={{
                  fontSize: 12,
                  color: "#888",
                  marginLeft: 10,
                  fontFamily: "monospace",
                }}
              >
                {vm.image}
              </span>
            </div>
            <div style={{ display: "flex", gap: 6 }}>
              {vm.socket_active && (
                <span
                  style={{
                    fontSize: 11,
                    padding: "3px 10px",
                    borderRadius: 6,
                    background: "#EAF3DE",
                    color: "#27500A",
                    fontWeight: 500,
                  }}
                >
                  virtiofsd up
                </span>
              )}
            </div>
          </div>

          {/* Métriques pull */}
          <div
            style={{
              display: "grid",
              gridTemplateColumns: "repeat(auto-fit, minmax(120px, 1fr))",
              gap: 8,
              marginBottom: 12,
            }}
          >
            <div
              style={{
                background: "#f8f8f8",
                borderRadius: 8,
                padding: "8px 12px",
              }}
            >
              <div style={{ fontSize: 11, color: "#999", marginBottom: 2 }}>
                téléchargé
              </div>
              <div style={{ fontSize: 14, fontWeight: 500 }}>
                {formatBytes(vm.bytes_downloaded)}
              </div>
            </div>
            <div
              style={{
                background: "#f8f8f8",
                borderRadius: 8,
                padding: "8px 12px",
              }}
            >
              <div style={{ fontSize: 11, color: "#999", marginBottom: 2 }}>
                total extrait
              </div>
              <div style={{ fontSize: 14, fontWeight: 500 }}>
                {formatBytes(vm.bytes_total)}
              </div>
            </div>
            <div
              style={{
                background: "#f8f8f8",
                borderRadius: 8,
                padding: "8px 12px",
              }}
            >
              <div style={{ fontSize: 11, color: "#999", marginBottom: 2 }}>
                durée pull
              </div>
              <div style={{ fontSize: 14, fontWeight: 500 }}>
                {formatDuration(vm.pull_duration_ms)}
              </div>
            </div>
            <div
              style={{
                background: "#f8f8f8",
                borderRadius: 8,
                padding: "8px 12px",
              }}
            >
              <div style={{ fontSize: 11, color: "#999", marginBottom: 2 }}>
                writes VM
              </div>
              <div style={{ fontSize: 14, fontWeight: 500 }}>
                {formatBytes(vm.upper_size)}
              </div>
            </div>
          </div>

          {/* Layers */}
          <div
            style={{
              display: "flex",
              gap: 4,
              flexWrap: "wrap",
              marginBottom: 6,
            }}
          >
            {vm.layers.map((l) => (
              <span
                key={l}
                style={{
                  fontSize: 11,
                  fontFamily: "monospace",
                  background: "#f5f5f5",
                  border: "0.5px solid #e0e0e0",
                  borderRadius: 4,
                  padding: "3px 8px",
                  color: "#555",
                }}
              >
                {l}
              </span>
            ))}
          </div>
          <div style={{ fontSize: 11, color: "#aaa", marginBottom: 8 }}>
            {vm.layers_downloaded} téléchargés · {vm.layers_cached} en cache
          </div>
          {/* Footer */}
          <div style={{ fontSize: 11, color: "#bbb" }}>
            pullé le {formatDate(vm.pulled_at)}
          </div>
        </div>
      ))}
    </section>
  );
}
