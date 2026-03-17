import { ColorField } from './ColorField'

export const FormBlock = {
  label: 'Form',
  fields: {
    action: { type: 'text', label: 'Form action URL' },
    formFields: {
      type: 'array',
      label: 'Form fields',
      arrayFields: {
        label: { type: 'text', label: 'Field label' },
        name: { type: 'text', label: 'Field name (used in DB, no spaces)' },
        fieldType: {
          type: 'select',
          label: 'Field type',
          options: [
            { label: 'Text input',  value: 'text' },
            { label: 'Email input', value: 'email' },
            { label: 'Checkbox',    value: 'checkbox' },
          ],
        },
        required: {
          type: 'custom',
          label: 'Required',
          render: ({ value, onChange }) => (
            <label style={{ display: 'flex', alignItems: 'center', gap: 8, fontSize: 13, cursor: 'pointer', padding: '4px 0' }}>
              <input type="checkbox" checked={!!value} onChange={e => onChange(e.target.checked)} />
              Required field
            </label>
          ),
        },
      },
      defaultItemProps: {
        label: 'Your name',
        name: 'name',
        fieldType: 'text',
        required: false,
      },
    },
    includeHuman: {
      type: 'custom',
      label: 'Include "I am human" checkbox',
      render: ({ value, onChange }) => (
        <label style={{ display: 'flex', alignItems: 'center', gap: 8, fontSize: 13, cursor: 'pointer', padding: '4px 0' }}>
          <input type="checkbox" checked={!!value} onChange={e => onChange(e.target.checked)} />
          Include "I am human" checkbox
        </label>
      ),
    },
    buttonLabel: { type: 'text', label: 'Button label' },
    buttonColor: {
      type: 'custom',
      label: 'Button color',
      render: ({ value, onChange }) => (
        <ColorField label="Button color" value={value ?? '#2563eb'} onChange={onChange} />
      ),
    },
    textColor: {
      type: 'custom',
      label: 'Text color',
      render: ({ value, onChange }) => (
        <ColorField label="Text color" value={value ?? '#111827'} onChange={onChange} />
      ),
    },
  },
  defaultProps: {
    action: '/form/contact',
    formFields: [
      { label: 'Your name',     name: 'name',    fieldType: 'text',  required: true },
      { label: 'Email address', name: 'email',   fieldType: 'email', required: true },
      { label: 'Message',       name: 'message', fieldType: 'text',  required: false },
    ],
    includeHuman: true,
    buttonLabel: 'Send Message',
    buttonColor: '#2563eb',
    textColor: '#111827',
  },
  render: ({ action, formFields, includeHuman, buttonLabel, buttonColor, textColor }) => (
    <div style={{ padding: '32px 40px', boxSizing: 'border-box', width: '100%' }}>
      <form action={action} method="POST" style={{ maxWidth: 640, margin: '0 auto', color: textColor }}>
        {(formFields || []).map((f, i) => (
          <div key={i} style={{ marginBottom: 16 }}>
            <label style={{ display: 'block', fontWeight: 600, marginBottom: 4, fontSize: 14 }}>
              {f.label}
            </label>
            {f.fieldType === 'checkbox' ? (
              <input type="checkbox" name={f.name || `field_${i}`} required={!!f.required} />
            ) : (
              <input
                type={f.fieldType === 'email' ? 'email' : 'text'}
                name={f.name || `field_${i}`}
                placeholder={f.label}
                required={!!f.required}
                style={{
                  width: '100%', padding: '10px 14px', fontSize: 14,
                  border: '1px solid #d1d5db', borderRadius: 6, boxSizing: 'border-box',
                }}
              />
            )}
          </div>
        ))}
        {includeHuman && (
          <div style={{ marginBottom: 16, display: 'flex', alignItems: 'center', gap: 8, fontSize: 14 }}>
            <input type="checkbox" id="human_check" required />
            <label htmlFor="human_check">I am human</label>
          </div>
        )}
        <button type="submit" style={{
          background: buttonColor, color: '#fff', border: 'none',
          padding: '12px 28px', borderRadius: 6, fontWeight: 600,
          fontSize: 15, cursor: 'pointer',
        }}>
          {buttonLabel}
        </button>
      </form>
    </div>
  ),
}
