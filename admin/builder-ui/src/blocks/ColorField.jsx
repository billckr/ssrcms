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

// ─── SHARED LAYOUT CONSTANTS ────────────────────────────────────────────────
// PADDING_OPTIONS and MAX_WIDTH_OPTIONS are the single source of truth for
// layout controls across ALL builder blocks. Every section-level block must
// import and use these — never define local copies with different values.
//
// This ensures that setting "Standard (1200px)" on a Hero, a Cards block,
// and a Paragraph block all produce the same max-width, so content edges
// visually align across the page.
//
// When adding a new top-level block:
//   1. import { ColorField, PADDING_OPTIONS, MAX_WIDTH_OPTIONS } from './ColorField'
//   2. Add a `padding` field using PADDING_OPTIONS
//   3. Add a `maxWidth` field using MAX_WIDTH_OPTIONS (default: '1200px')
//   4. Apply them in the render: outer div gets `padding`, inner content
//      div gets `maxWidth` + `margin: '0 auto'`
//   5. Mirror both in the matching Tera template using
//      `{{ block_config.padding | default(value='32px 40px') }}` and
//      `{{ block_config.maxWidth | default(value='1200px') }}`
//
// Blocks dropped inside zones (Text, etc.) do NOT need these — they inherit
// layout from their container (Cards, Columns, etc.).
// ────────────────────────────────────────────────────────────────────────────

export const PADDING_OPTIONS = [
  { label: 'None',   value: '0px' },
  { label: 'Small',  value: '16px 24px' },
  { label: 'Medium', value: '32px 40px' },
  { label: 'Large',  value: '64px 80px' },
]

export const MAX_WIDTH_OPTIONS = [
  { label: 'Full (100%)',        value: '100%' },
  { label: 'Wide (1400px)',      value: '1400px' },
  { label: 'Standard (1200px)', value: '1200px' },
  { label: 'Medium (960px)',     value: '960px' },
  { label: 'Narrow (720px)',     value: '720px' },
]
