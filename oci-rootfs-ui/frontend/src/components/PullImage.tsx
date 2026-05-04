import { useState } from 'react'
import { useQuery, useQueryClient } from '@tanstack/react-query'
import axios from 'axios'

interface PullStatus {
  name: string
  status: string
  error?: string
}

export function PullImage() {
  const [image, setImage] = useState('')
  const [name, setName] = useState('')
  const [loading, setLoading] = useState(false)
  const qc = useQueryClient()

  const { data: pulls = [] } = useQuery({
    queryKey: ['pulls'],
    queryFn: () => axios.get<PullStatus[]>('/api/pulls').then(r => r.data)
  })

  async function handlePull() {
    if (!image || !name) return
    setLoading(true)
    await axios.post('/api/pull', { image, name })
    setImage('')
    setName('')
    setLoading(false)
    qc.invalidateQueries({ queryKey: ['pulls'] })
  }

  return (
    <section style={{ marginBottom: '2rem' }}>
      <h2 style={{ fontSize: 13, fontWeight: 500, color: '#888', textTransform: 'uppercase', letterSpacing: '0.05em', marginBottom: 10 }}>
        Pull image
      </h2>
      <div style={{ display: 'flex', gap: 8, marginBottom: 12 }}>
        <input
          placeholder="debian:trixie-slim"
          value={image}
          onChange={e => setImage(e.target.value)}
          style={{ flex: 2, padding: '8px 12px', borderRadius: 8, border: '0.5px solid #ddd', fontSize: 13, fontFamily: 'monospace' }}
        />
        <input
          placeholder="vm-name"
          value={name}
          onChange={e => setName(e.target.value)}
          style={{ flex: 1, padding: '8px 12px', borderRadius: 8, border: '0.5px solid #ddd', fontSize: 13, fontFamily: 'monospace' }}
        />
        <button
          onClick={handlePull}
          disabled={loading || !image || !name}
          style={{
            padding: '8px 20px',
            borderRadius: 8,
            border: '0.5px solid #ddd',
            background: loading ? '#f0f0f0' : '#fff',
            cursor: loading ? 'not-allowed' : 'pointer',
            fontSize: 13,
            fontWeight: 500
          }}
        >
          {loading ? 'pulling...' : 'pull ↗'}
        </button>
      </div>
      {pulls.length > 0 && (
        <div style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
          {pulls.map(p => (
            <div key={p.name} style={{
              display: 'flex',
              alignItems: 'center',
              gap: 10,
              fontSize: 13,
              padding: '6px 12px',
              borderRadius: 8,
              background: p.status === 'error' ? '#fff5f5' : p.status === 'ready' ? '#f0faf0' : '#f5f5ff'
            }}>
              <span style={{ fontFamily: 'monospace', fontWeight: 500 }}>{p.name}</span>
              <span style={{ color: p.status === 'error' ? '#c00' : p.status === 'ready' ? '#270' : '#55a' }}>
                {p.status}
              </span>
              {p.error && <span style={{ color: '#c00', fontSize: 11 }}>{p.error}</span>}
            </div>
          ))}
        </div>
      )}
    </section>
  )
}