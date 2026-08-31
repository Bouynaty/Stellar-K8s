import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

export default defineConfig({
  plugins: [react()],
  server: {
    port: 5174,
    strictPort: false,
    proxy: {
      '/api/prometheus': {
        target: 'http://localhost:9091',
        changeOrigin: true,
        rewrite: (path) => path.replace(/^\/api\/prometheus/, ''),
      },
      '/api': {
        target: 'http://localhost:9090',
        ws: true,
      },
      '/ws': {
        target: 'ws://localhost:9090',
        ws: true,
      },
    },
  },
});
