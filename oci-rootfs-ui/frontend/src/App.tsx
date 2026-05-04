import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { VmList } from './components/VmList'
import { BlobCache } from './components/BlobCache'
import { PullImage } from './components/PullImage'

const queryClient = new QueryClient({
  defaultOptions: { queries: { refetchInterval: 2000 } }
})

export default function App() {
  return (
    <QueryClientProvider client={queryClient}>
      <div style={{ maxWidth: 900, margin: '0 auto', padding: '2rem 1rem' }}>
        <header style={{ marginBottom: '2rem' }}>
          <h1 style={{ fontSize: 22, fontWeight: 500, margin: 0 }}>oci-rootfs</h1>
          <p style={{ fontSize: 13, color: '#888', marginTop: 4 }}>
            ~/.local/share/kyernal/
          </p>
        </header>
        <PullImage />
        <VmList />
        <BlobCache />
      </div>
    </QueryClientProvider>
  )
}