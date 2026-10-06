import { defineConfig } from 'vite';

export default defineConfig({
  server: {
    port: 1420,
    strictPort: true,
  },
  clearScreen: false,
  build: {
    target: 'esnext',
    // Vite 8 基于 rolldown，内置压缩器由 esbuild 切换为 oxc；
    // 旧的 'esbuild' 已被移除且需要额外安装 esbuild，构建会报
    // transformWithEsbuild / Cannot find package 'esbuild'。
    minify: 'oxc',
  },
});
