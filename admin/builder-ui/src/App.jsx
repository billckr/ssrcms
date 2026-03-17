import { Puck } from '@measured/puck'
import '@measured/puck/puck.css'
import { useState, useEffect } from 'react'
import { HeroBlock } from './blocks/Hero'

const config = {
  components: {
    Hero: HeroBlock,
  },
}

const init = window.__builderInit || {}
const SITE_ID    = init.siteId    || ''
const PROJECT_ID = init.projectId || ''
const PAGE_ID    = init.pageId    || null

export default function App() {
  const [initialData, setInitialData] = useState(null)
  const [saving, setSaving]           = useState(false)
  const [saveError, setSaveError]     = useState(null)

  useEffect(() => {
    if (!PAGE_ID) { setInitialData({}); return }
    fetch(`/admin/builder/load/${PAGE_ID}`)
      .then(r => r.ok ? r.json() : {})
      .then(data => setInitialData(data))
      .catch(() => setInitialData({}))
  }, [])

  async function handlePublish(data) {
    const name = document.getElementById('page-name')?.value?.trim() || 'Untitled'
    setSaving(true)
    setSaveError(null)
    try {
      const res = await fetch('/admin/builder/save', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          site_id:        SITE_ID,
          project_id:     PROJECT_ID,
          composition_id: PAGE_ID,
          name,
          data,
        }),
      })
      if (!res.ok) throw new Error(`Server error ${res.status}`)
      window.location.href = `/admin/builder/${PROJECT_ID}`
    } catch (err) {
      setSaveError('Save failed — ' + err.message)
      setSaving(false)
    }
  }

  if (initialData === null) {
    return (
      <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'center', height: '100vh', background: '#09090b', color: '#888' }}>
        Loading…
      </div>
    )
  }

  return (
    <div style={{ height: '100vh', display: 'flex', flexDirection: 'column' }}>
      <div style={{
        display: 'flex', alignItems: 'center', gap: 12,
        padding: '8px 16px', background: '#18181b', borderBottom: '1px solid #333', flexShrink: 0,
      }}>
        <a href={`/admin/builder/${PROJECT_ID}`} style={{ color: '#888', textDecoration: 'none', fontSize: 13 }}>
          ← Back
        </a>
        <input
          id="page-name"
          defaultValue={init.pageName || ''}
          placeholder="Page name…"
          style={{
            background: '#27272a', border: '1px solid #3f3f46', borderRadius: 4,
            color: '#fff', padding: '4px 10px', fontSize: 14, width: 220,
          }}
        />
        <span style={{ flex: 1 }} />
        {saveError && <span style={{ color: '#f87171', fontSize: 12 }}>{saveError}</span>}
        {saving   && <span style={{ color: '#888',   fontSize: 12 }}>Saving…</span>}
        <span style={{ color: '#555', fontSize: 12 }}>Synaptic Signals — Page Builder</span>
      </div>
      <div style={{ flex: 1, overflow: 'hidden' }}>
        <Puck config={config} onPublish={handlePublish} data={initialData} />
      </div>
    </div>
  )
}
