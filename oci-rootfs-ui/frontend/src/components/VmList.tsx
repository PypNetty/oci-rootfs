import { useQuery } from '@tanstack/react-query'
import axios from 'axios'

interface VmInfo {
  name: string
  layers: string[]
  socket_active: boolean
  upper_size: number
}

function formatBytes(b: number) {
  if (b < 1024) return `${b} B`
  if (b < 1024 * 1024) return `${(b / 1024).toFixed(1)} KB`
  return `${(b / 1024 / 1024).toFixed(1)} MB`
}

export function VmList() {
  const { data: vms = [], isLoading } = useQuery({
    queryKey: ['vms'],
    queryFn: () => axios.get<VmInfo[]>('/api/vms').then(r => r.data)
  })

  return (
    <section style={{ marginBottom: '2rem' }}>
      <h2 style={{ fontSize: 13, fontWeight: 500, color: '#888', textTransform: 'uppercase', letterSpacing: '0.05em', marginBottom: 10 }}>
        VMs — {vms.length}
      </h2>
      {isLoading && <p style={{ fontSize: 13, color: '#888' }}>chargement...</p>}
      {vms.map(vm => (
        <div key={vm.name} style={{
          background: '#fff',
          border: '0.5px solid #e5e5e5',
          borderRadius: 12,
          padding: '1rem 1.25rem',
          marginBottom: 10
        }}>
          <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 10 }}>
            <span style={{ fontFamily: 'monospace', fontSize: 15, fontWeight: 500 }}>{vm.name}</span>
            <div style={{ display: 'flex', gap: 6 }}>
              {vm.socket_active && (
                <span style={{ fontSize: 11, padding: '3px 10px', borderRadius: 6, background: '#EAF3DE', color: '#27500A', fontWeight: 500 }}>
                  virtiofsd up
                </span>
              )}
              <span style={{ fontSize: 11, padding: '3px 10px', borderRadius: 6, background: '#f0f0f0', color: '#555' }}>
                {vm.layers.length} layer{vm.layers.length > 1 ? 's' : ''}
              </span>
              <span style={{ fontSize: 11, padding: '3px 10px', borderRadius: 6, background: '#f0f0f0', color: '#555' }}>
                upper {formatBytes(vm.upper_size)}
              </span>
            </div>
          </div>
          <div style={{ display: 'flex', gap: 4, flexWrap: 'wrap' }}>
            {vm.layers.map(l => (
              <span key={l} style={{
                fontSize: 11,
                fontFamily: 'monospace',
                background: '#f5f5f5',
                border: '0.5px solid #e0e0e0',
                borderRadius: 4,
                padding: '3px 8px',
                color: '#555'
              }}>
                {l.slice(0, 12)}
              </span>
            ))}
          </div>
        </div>
      ))}
    </section>
  )
}