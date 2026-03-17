import { Puck } from '@measured/puck'
import '@measured/puck/puck.css'
import { HeroBlock } from './blocks/Hero'

const config = {
  components: {
    Hero: HeroBlock,
  },
}

const init = window.__builderInit || {}
const SITE_ID = init.siteId || ''
const COMPOSITION_ID = init.compositionId || null

async function loadData() {
  if (!COMPOSITION_ID) return {}
  const res = await fetch(`/admin/builder/load/${COMPOSITION_ID}`)
  if (!res.ok) return {}
  return res.json()
}

async function saveData(data) {
  const name = document.getElementById('composition-name')?.value || 'Untitled'
  await fetch('/admin/builder/save', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ site_id: SITE_ID, composition_id: COMPOSITION_ID, name, data }),
  })
}

export default function App() {
  return (
    <div style={{ height: '100vh', display: 'flex', flexDirection: 'column' }}>
      <div style={{
        display: 'flex', alignItems: 'center', gap: 12,
        padding: '8px 16px', background: '#18181b', borderBottom: '1px solid #333',
      }}>
        <a href="/admin/appearance" style={{ color: '#888', textDecoration: 'none', fontSize: 13 }}>
          ← Back
        </a>
        <input
          id="composition-name"
          defaultValue={init.compositionName || ''}
          placeholder="Page name…"
          style={{
            background: '#27272a', border: '1px solid #3f3f46', borderRadius: 4,
            color: '#fff', padding: '4px 10px', fontSize: 14, width: 220,
          }}
        />
        <span style={{ flex: 1 }} />
        <span style={{ color: '#555', fontSize: 12 }}>
          Synaptic Signals — Page Builder
        </span>
      </div>
      <div style={{ flex: 1, overflow: 'hidden' }}>
        <Puck
          config={config}
          onPublish={saveData}
          data={loadData}
        />
      </div>
    </div>
  )
}
