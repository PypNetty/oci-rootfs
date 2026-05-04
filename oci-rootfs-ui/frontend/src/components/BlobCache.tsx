import { useQuery, useQueryClient } from "@tanstack/react-query";
import axios from "axios";

interface BlobInfo {
  digest: string;
  size: number;
  extracted_size: number;
  duration_ms: number;
  pulled_at: string;
  expires_at: string;
  ttl_hours: number;
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

function ExpiryBadge({ expires_at }: { expires_at: string }) {
  if (!expires_at)
    return <span style={{ fontSize: 11, color: "#bbb" }}>—</span>;

  const now = Date.now();
  const exp = new Date(expires_at).getTime();
  const diff = exp - now;
  const hours = Math.floor(diff / 3600000);
  const expired = diff < 0;

  return (
    <span
      style={{
        fontSize: 11,
        padding: "2px 8px",
        borderRadius: 4,
        background: expired ? "#fff5f5" : hours < 6 ? "#FAEEDA" : "#EAF3DE",
        color: expired ? "#c00" : hours < 6 ? "#633806" : "#27500A",
        fontWeight: 500,
      }}
    >
      {expired ? "expiré" : hours < 1 ? "< 1h" : `${hours}h`}
    </span>
  );
}

export function BlobCache() {
  const qc = useQueryClient();
  const { data: blobs = [] } = useQuery({
    queryKey: ["blobs"],
    queryFn: () => axios.get<BlobInfo[]>("/api/blobs").then((r) => r.data),
  });

  async function handleDelete(digest: string) {
    await axios.delete(`/api/blobs/${digest}`);
    qc.invalidateQueries({ queryKey: ["blobs"] });
  }

  const totalExtracted = blobs.reduce((acc, b) => acc + b.extracted_size, 0);
  const totalRaw = blobs.reduce((acc, b) => acc + b.size, 0);

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
        Blobs — {blobs.length} · {formatBytes(totalRaw)} raw ·{" "}
        {formatBytes(totalExtracted)} extrait
      </h2>

      <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
        {blobs.map((blob) => (
          <div
            key={blob.digest}
            style={{
              background: "#fff",
              border: "0.5px solid #e5e5e5",
              borderRadius: 8,
              padding: "10px 14px",
              display: "grid",
              gridTemplateColumns: "1fr auto",
              alignItems: "center",
              gap: 12,
            }}
          >
            <div>
              <div
                style={{
                  fontFamily: "monospace",
                  fontSize: 12,
                  color: "#444",
                  marginBottom: 6,
                }}
              >
                {blob.digest.slice(0, 24)}...
              </div>
              <div style={{ display: "flex", gap: 16, flexWrap: "wrap" }}>
                <span style={{ fontSize: 11, color: "#999" }}>
                  raw{" "}
                  <strong style={{ color: "#555" }}>
                    {formatBytes(blob.size)}
                  </strong>
                </span>
                <span style={{ fontSize: 11, color: "#999" }}>
                  extrait{" "}
                  <strong style={{ color: "#555" }}>
                    {formatBytes(blob.extracted_size)}
                  </strong>
                </span>
                <span style={{ fontSize: 11, color: "#999" }}>
                  pull{" "}
                  <strong style={{ color: "#555" }}>
                    {formatDuration(blob.duration_ms)}
                  </strong>
                </span>
                <span style={{ fontSize: 11, color: "#999" }}>
                  {formatDate(blob.pulled_at)}
                </span>
              </div>
            </div>

            <div
              style={{
                display: "flex",
                flexDirection: "column",
                alignItems: "flex-end",
                gap: 6,
              }}
            >
              <ExpiryBadge expires_at={blob.expires_at} />
              {blob.ttl_hours > 0 && (
                <span style={{ fontSize: 10, color: "#ccc" }}>
                  TTL {blob.ttl_hours}h
                </span>
              )}
              <button
                onClick={() => handleDelete(blob.digest)}
                style={{
                  fontSize: 11,
                  padding: "2px 8px",
                  borderRadius: 4,
                  border: "0.5px solid #f5c6c6",
                  background: "#fff5f5",
                  color: "#c00",
                  cursor: "pointer",
                }}
              >
                supprimer
              </button>
            </div>
          </div>
        ))}
      </div>
    </section>
  );
}
