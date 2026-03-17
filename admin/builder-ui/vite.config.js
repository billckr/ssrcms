import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

export default defineConfig({
  plugins: [react()],
  build: {
    outDir: '../static/builder',
    emptyOutDir: true,
    rollupOptions: {
      output: {
        entryFileNames: 'builder.js',
        chunkFileNames: 'builder-[name].js',
        assetFileNames: (info) => {
          const name = info.names?.[0] ?? info.name ?? ''
          return name.endsWith('.css') ? 'builder.css' : '[name][extname]'
        },
      },
    },
  },
  base: '/admin/static/builder/',
})
