/**
 * Gera uma versão do santuário em UM único arquivo HTML.
 *
 * Tudo entra embutido — Three.js, os módulos do domínio e o CSS — para a
 * página rodar de qualquer lugar (pen drive, file://, hospedagem estática ou
 * um host que bloqueie requisições externas).
 *
 *   node tools/build-standalone.mjs
 *
 * Saídas em dist-standalone/:
 *   malevolent-shrine.html   página completa, pronta para abrir no navegador
 *   artifact.html            só o conteúdo, para hosts que fornecem o esqueleto
 */
import { build } from 'vite';
import { mkdir, readdir, readFile, writeFile, rm } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import path from 'node:path';

const root = fileURLToPath(new URL('..', import.meta.url));
const outDir = path.join(root, 'dist-standalone');
const tmpDir = path.join(outDir, '.tmp');

const FONTS = [
  '<link rel="preconnect" href="https://fonts.googleapis.com">',
  '<link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>',
  '<link href="https://fonts.googleapis.com/css2?family=Inter:wght@400;500;600&family=Noto+Serif+JP:wght@500;700&display=swap" rel="stylesheet">',
].join('\n    ');

async function main() {
  await rm(outDir, { recursive: true, force: true });
  await mkdir(tmpDir, { recursive: true });

  // Modo "lib" + IIFE: o Vite junta Three.js e todos os módulos num arquivo
  // só, e o CSS sai à parte para ser embutido em seguida.
  await build({
    configFile: false,
    root,
    logLevel: 'warn',
    build: {
      outDir: tmpDir,
      emptyOutDir: true,
      cssCodeSplit: false,
      target: 'es2020',
      lib: {
        entry: path.join(root, 'src/shrine/main.js'),
        formats: ['iife'],
        name: 'MalevolentShrine',
        fileName: () => 'bundle.js',
      },
    },
  });

  // O Vite nomeia o CSS a partir do package.json, então procuramos pela extensão
  const emitted = await readdir(tmpDir);
  const cssName = emitted.find((name) => name.endsWith('.css'));
  if (!cssName) throw new Error('build sem CSS: nada para embutir');

  const js = await readFile(path.join(tmpDir, 'bundle.js'), 'utf8');
  const css = await readFile(path.join(tmpDir, cssName), 'utf8');
  const page = await readFile(path.join(root, 'malevolent-shrine.html'), 'utf8');

  // Corpo da página, sem o esqueleto do documento
  const body = page
    .slice(page.indexOf('<body>') + '<body>'.length, page.lastIndexOf('</body>'))
    .replace(/\n\s*<script[\s\S]*?<\/script>/g, '')
    .trim();

  const title = '伏魔御廚子 · Malevolent Shrine — Simulação 3D';
  const description =
    'Simulação 3D interativa do Malevolent Shrine (伏魔御廚子), o Domínio de Ryomen Sukuna. Geometria 100% procedural em Three.js.';

  const inline = `<style>\n${css}\n</style>\n\n${body}\n\n<script>\n${js}\n</script>\n`;

  const full = `<!doctype html>
<html lang="pt-BR">
  <head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0, viewport-fit=cover">
    <meta name="description" content="${description}">
    <meta name="theme-color" content="#0a0304">
    <title>${title}</title>
    ${FONTS}
  </head>
  <body>
${inline}
  </body>
</html>
`;

  const artifact = `<title>${title}</title>
${FONTS}

${inline}`;

  await writeFile(path.join(outDir, 'malevolent-shrine.html'), full);
  await writeFile(path.join(outDir, 'artifact.html'), artifact);
  await rm(tmpDir, { recursive: true, force: true });

  const kb = (text) => `${(Buffer.byteLength(text) / 1024).toFixed(0)} kB`;
  console.log(`dist-standalone/malevolent-shrine.html  ${kb(full)}`);
  console.log(`dist-standalone/artifact.html           ${kb(artifact)}`);
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
