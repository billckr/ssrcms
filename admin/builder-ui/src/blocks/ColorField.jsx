import { HexColorPicker } from 'react-colorful'
import { useState } from 'react'

export function ColorField({ label, value, onChange }) {
  const [open, setOpen] = useState(false)
  const [hex, setHex] = useState(value || '#ffffff')

  const safe = value || '#ffffff'

  const handleHexInput = (e) => {
    const v = e.target.value
    setHex(v)
    if (/^#[0-9a-fA-F]{6}$/.test(v)) {
      onChange(v)
    }
  }

  const handlePickerChange = (v) => {
    setHex(v)
    onChange(v)
  }

  return (
    <div style={{ marginBottom: 12 }}>
      {label && (
        <label style={{ display: 'block', fontSize: 11, fontWeight: 600, marginBottom: 4, textTransform: 'uppercase', color: '#888' }}>{label}</label>
      )}
      <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
        <div style={{ position: 'relative', display: 'inline-block' }}>
          <div
            onClick={() => setOpen(!open)}
            style={{ width: 32, height: 32, borderRadius: 4, cursor: 'pointer', background: safe, border: '2px solid #ccc' }}
          />
          {open && (
            <div style={{ position: 'absolute', zIndex: 1000, top: 36, left: 0 }}>
              <HexColorPicker color={safe} onChange={handlePickerChange} />
              <button
                onClick={() => setOpen(false)}
                style={{ marginTop: 8, width: '100%', cursor: 'pointer' }}
              >
                Close
              </button>
            </div>
          )}
        </div>
        <input
          type="text"
          value={hex}
          onChange={handleHexInput}
          maxLength={7}
          placeholder="#ffffff"
          style={{ width: 80, fontSize: 13, padding: '4px 6px', border: '1px solid #ccc', borderRadius: 4, fontFamily: 'monospace' }}
        />
      </div>
    </div>
  )
}

export const PADDING_OPTIONS = [
  { label: 'None',   value: '0px' },
  { label: 'Small',  value: '16px 24px' },
  { label: 'Medium', value: '32px 40px' },
  { label: 'Large',  value: '64px 80px' },
]
