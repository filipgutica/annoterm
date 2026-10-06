import { defineConfig } from 'astro/config';
import vue from '@astrojs/vue';
import tailwindcss from '@tailwindcss/vite';

export default defineConfig({
  site: 'https://filipgutica.github.io',
  base: '/annoterm',
  output: 'static',
  trailingSlash: 'always',
  integrations: [vue()],
  vite: { plugins: [tailwindcss()] },
});
