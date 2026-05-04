import { useQuery } from '@tanstack/react-query'
import axios from 'axios'

interface BlobInfo {
  digest: string
  size: number
  extracted_size: number
}

function formatBytes(b: number) {
  if (b < 1024) return `${b} B`
  if (b < 1024 * 1024) return `${(b / 1024).toFixed(1)} KB`
  return `${(b / 1024 / 1024).toFixed(1)} MB`
}

export function BlobCache() {
  const { data: blobs = [] } = useQuery({
    queryKey: ['blobs'],
    queryFn: () => axios.get<BlobInfo[]>('/api/blobs').then(r => r.data)
  })

  const totalExtracted = blobs.reduce((acc, b) => acc + b.extracted_size, 0)

  return (
    <section style={{ marginBottom: '2rem' }}>
      <h2 style={{ fontSize: 13, fontWeight: 500, color: '#888', textTransform: 'uppercase', letterSpacing: '0.05em', marginBottom: 10 }}>
        Blobs — {blobs.length} · {formatBytes(totalExtracted)} extrait
      </h2>
      <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fill, minmax(200px, 1fr))', gap: 8 }}>
        {blobs.map(blob => (
          <div key={blob.digest} style={{
            background: '#f8f8f8',
            borderRadius: 8,
            padding: '8px 12px',
            border: '0.5px solid #e5e5e5'
          }}>
            <div style={{ fontFamily: 'monospace', fontSize: 12, color: '#444' }}>
              {blob.digest.slice(0, 16)}...
            </div>
            <div style={{ fontSize: 11, color: '#999', marginTop: 4 }}>
              raw {formatBytes(blob.size)} · ext {formatBytes(blob.extracted_size)}
            </div>
          </div>
        ))}
      </div>
    </section>
  )
}