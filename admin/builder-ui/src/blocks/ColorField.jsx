import { HexColorPicker as ColorPicker } from 'react-colorful'
import { useState } from 'react'

export function ColorField({ label, value, onChange }) {
  const [open, setOpen] = useState(false)
  return (
    <div style={{ marginBottom: 12 }}>
      <label style={{ display: 'block', fontSize: 11, fontWeight: 600, marginBottom: 4, textTransform: 'uppercase', color: '#888' }}>{label}</label>
      <div style={{ position: 'relative', display: 'inline-block' }}>
        <div
          onClick={() => setOpen(o => !o)}
          style={{ width: 32, height: 32, borderRadius: 4, background: value, border: '2px solid #444', cursor: 'pointer' }}
        />
        {open && (
          <div style={{ position: 'absolute', zIndex: 100, top: 36, left: 0 }}>
            <ColorPicker color={value} onChange={onChange} />
            <button onClick={() => setOpen(false)} style={{ width: '100%', marginTop: 4, background: '#333', border: 'none', color: '#fff', borderRadius: 4, padding: '4px 0', cursor: 'pointer', fontSize: 12 }}>Done</button>
          </div>
        )}
      </div>
      <input value={value} onChange={e => onChange(e.target.value)} style={{ marginLeft: 8, width: 90, background: '#1e1e1e', border: '1px solid #444', borderRadius: 4, color: '#fff', padding: '4px 8px', fontSize: 12 }} />
    </div>
  )
}

export const PADDING_OPTIONS = [
  { label: 'None',   value: '0px' },
  { label: 'Small',  value: '16px 24px' },
  { label: 'Medium', value: '32px 40px' },
  { label: 'Large',  value: '64px 80px' },
]
