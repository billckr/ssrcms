import { Puck } from '@measured/puck'
import '@measured/puck/puck.css'
import { useState, useEffect, useRef, useCallback } from 'react'
import { HeroBlock } from './blocks/Hero'
import { HeaderBlock } from './blocks/Header'
import { FooterBlock } from './blocks/Footer'
import { ButtonBlock } from './blocks/Button'
import { ColumnsBlock } from './blocks/Columns'

const config = {
  components: {
    Hero: HeroBlock,
    Header: HeaderBlock,
    Footer: FooterBlock,
    Button: ButtonBlock,
    Columns: ColumnsBlock,
  },
}

const init = window.__builderInit || {}
const SITE_ID    = init.siteId    || ''
const PROJECT_ID = init.projectId || ''
const PAGE_ID    = init.pageId    || null

const AUTO_SAVE_MS = 30_000

const STATUS_COLOR = { dirty: '#92400e', saving: '#64748b', saved: '#166534', error: '#b91c1c' }

function setStatus(text, type) {
  const el = document.getElementById('status-msg')
  if (!el) return
  el.textContent = text
  el.style.color = STATUS_COLOR[type] || ''
}

export default function App() {
  const [initialData, setInitialData] = useState(null)
  const [currentData, setCurrentData] = useState(null)
  const [isDirty, setIsDirty]         = useState(false)
  const [saving, setSaving]           = useState(false)
  const autoSaveTimer                 = useRef(null)

  useEffect(() => {
    if (!PAGE_ID) { setInitialData({}); return }
    fetch(`/admin/builder/load/${PAGE_ID}`)
      .then(r => r.ok ? r.json() : {})
      .then(data => setInitialData(data))
      .catch(() => setInitialData({}))
  }, [])

  useEffect(() => {
    const btn = document.getElementById('back-btn')
    if (!btn) return
    const handler = (e) => { if (isDirty && !confirm('You have unsaved changes. Leave anyway?')) e.preventDefault() }
    btn.addEventListener('click', handler)
    return () => btn.removeEventListener('click', handler)
  }, [isDirty])

  useEffect(() => {
    const handler = (e) => { if (!isDirty) return; e.preventDefault(); e.returnValue = '' }
    window.addEventListener('beforeunload', handler)
    return () => window.removeEventListener('beforeunload', handler)
  }, [isDirty])

  useEffect(() => {
    if (!isDirty || !currentData) return
    clearTimeout(autoSaveTimer.current)
    autoSaveTimer.current = setTimeout(() => doSave(currentData, true), AUTO_SAVE_MS)
    return () => clearTimeout(autoSaveTimer.current)
  }, [currentData, isDirty])

  const doSave = useCallback(async (data, isAuto = false) => {
    const name = init.pageName || 'Untitled'
    setSaving(true)
    setStatus('Saving…', 'saving')
    try {
      const res = await fetch('/admin/builder/save', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ site_id: SITE_ID, project_id: PROJECT_ID, composition_id: PAGE_ID, name, data }),
      })
      if (!res.ok) throw new Error(`Server error ${res.status}`)
      setIsDirty(false)
      setStatus('Draft saved', 'saved')
      setTimeout(() => setStatus('', ''), 3000)
    } catch (err) {
      if (!isAuto) setStatus('Save failed — ' + err.message, 'error')
    } finally {
      setSaving(false)
    }
  }, [])

  function handleChange(data) {
    setCurrentData(data)
    setIsDirty(true)
    setStatus('Unsaved changes', 'dirty')
  }

  async function handlePublish(data) {
    if (!data.content || data.content.length === 0) {
      setStatus('Add at least one block before publishing.', 'error')
      return
    }
    const name = init.pageName || 'Untitled'
    setSaving(true)
    setStatus('Publishing…', 'saving')
    try {
      const res = await fetch('/admin/builder/publish', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ site_id: SITE_ID, project_id: PROJECT_ID, composition_id: PAGE_ID, name, data }),
      })
      if (!res.ok) throw new Error(`Server error ${res.status}`)
      setIsDirty(false)
      setStatus('Published!', 'saved')
      setTimeout(() => setStatus('', ''), 4000)
    } catch (err) {
      setStatus('Publish failed — ' + err.message, 'error')
    } finally {
      setSaving(false)
    }
  }

  if (initialData === null) {
    return (
      <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'center', height: '100%', background: '#f8fafc', color: '#64748b' }}>
        Loading…
      </div>
    )
  }

  const overrides = {
    headerActions: ({ children }) => (
      <>
        <button
          onClick={() => doSave(currentData || initialData || {})}
          disabled={saving}
          style={{
            background: '#fff', border: '1px solid #e2e8f0', borderRadius: 4,
            color: '#475569', padding: '6px 14px', fontSize: 13,
            fontWeight: 500, cursor: saving ? 'not-allowed' : 'pointer', marginRight: 4,
          }}
        >
          Save Draft
        </button>
        {children}
      </>
    ),
  }

  return (
    <Puck config={config} onPublish={handlePublish} data={initialData} onChange={handleChange} overrides={overrides} />
  )
}
