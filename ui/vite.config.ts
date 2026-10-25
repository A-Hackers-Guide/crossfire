import react from '@vitejs/plugin-react'
import { defineConfig } from 'vite'

// Relative base so the same build loads from Tauri, Electron (file://) and the web.
export default defineConfig({
  base: './',
  plugins: [react()],
  server: { strictPort: true },
  build: { target: 'es2022', chunkSizeWarningLimit: 1200 },
})
