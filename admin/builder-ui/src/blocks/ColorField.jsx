export function ColorField({ label, value, onChange }) {
  const safe = value || '#ffffff'
  return (
    <div style={{ marginBottom: 12 }}>
      <label style={{ display: 'block', fontSize: 11, fontWeight: 600, marginBottom: 4, textTransform: 'uppercase', color: '#888' }}>{label}</label>
      <input
        type="color"
        value={safe}
        onChange={e => onChange(e.target.value)}
        style={{ width: 40, height: 32, padding: 2, border: '2px solid #ccc', borderRadius: 4, background: 'none', cursor: 'pointer' }}
      />
    </div>
  )
}

export const PADDING_OPTIONS = [
  { label: 'None',   value: '0px' },
  { label: 'Small',  value: '16px 24px' },
  { label: 'Medium', value: '32px 40px' },
  { label: 'Large',  value: '64px 80px' },
]
