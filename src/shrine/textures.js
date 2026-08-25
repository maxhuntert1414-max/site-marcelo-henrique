/**
 * Texturas procedurais para o Domínio.
 * Nada de arquivos externos: tudo é gerado em runtime com ruído de valor + fBm.
 */
import * as THREE from 'three';

const lerp = (a, b, t) => a + (b - a) * t;
const clamp01 = (v) => (v < 0 ? 0 : v > 1 ? 1 : v);

function hash(x, y, seed) {
  let h = Math.imul(x | 0, 374761393) + Math.imul(y | 0, 668265263) + Math.imul(seed | 0, 1442695041);
  h = Math.imul(h ^ (h >>> 13), 1274126177);
  return ((h ^ (h >>> 16)) >>> 0) / 4294967295;
}

/** Ruído de valor com período inteiro — permite textura contínua nas bordas. */
function valueNoise(x, y, period, seed) {
  const xi = Math.floor(x);
  const yi = Math.floor(y);
  const xf = x - xi;
  const yf = y - yi;
  const u = xf * xf * (3 - 2 * xf);
  const v = yf * yf * (3 - 2 * yf);
  const wrap = (n) => ((n % period) + period) % period;
  const x0 = wrap(xi);
  const x1 = wrap(xi + 1);
  const y0 = wrap(yi);
  const y1 = wrap(yi + 1);

  return lerp(
    lerp(hash(x0, y0, seed), hash(x1, y0, seed), u),
    lerp(hash(x0, y1, seed), hash(x1, y1, seed), u),
    v,
  );
}

function fbm(x, y, basePeriod, octaves, seed) {
  let sum = 0;
  let norm = 0;
  let amp = 0.5;
  let freq = basePeriod;

  for (let o = 0; o < octaves; o += 1) {
    sum += amp * valueNoise(x * freq, y * freq, freq, seed + o * 137);
    norm += amp;
    amp *= 0.5;
    freq *= 2;
  }

  return sum / norm;
}

/** Ruído "ridged": cria veios e rachaduras finas. */
function ridged(x, y, basePeriod, octaves, seed) {
  const n = fbm(x, y, basePeriod, octaves, seed);
  return 1 - Math.abs(n * 2 - 1);
}

function makeDataTexture(data, size, { colorSpace, repeat = 1 } = {}) {
  const texture = new THREE.DataTexture(data, size, size, THREE.RGBAFormat);
  texture.wrapS = THREE.RepeatWrapping;
  texture.wrapT = THREE.RepeatWrapping;
  texture.repeat.set(repeat, repeat);
  texture.magFilter = THREE.LinearFilter;
  texture.minFilter = THREE.LinearMipmapLinearFilter;
  texture.generateMipmaps = true;
  texture.anisotropy = 8;
  if (colorSpace) texture.colorSpace = colorSpace;
  texture.needsUpdate = true;
  return texture;
}

/** Converte um campo de altura em normal map tangente (Sobel). */
function heightToNormal(height, size, strength) {
  const data = new Uint8Array(size * size * 4);
  const at = (x, y) => height[((y + size) % size) * size + ((x + size) % size)];

  for (let y = 0; y < size; y += 1) {
    for (let x = 0; x < size; x += 1) {
      const dx =
        at(x - 1, y - 1) + 2 * at(x - 1, y) + at(x - 1, y + 1) -
        (at(x + 1, y - 1) + 2 * at(x + 1, y) + at(x + 1, y + 1));
      const dy =
        at(x - 1, y - 1) + 2 * at(x, y - 1) + at(x + 1, y - 1) -
        (at(x - 1, y + 1) + 2 * at(x, y + 1) + at(x + 1, y + 1));

      let nx = dx * strength;
      let ny = dy * strength;
      const nz = 1;
      const len = Math.hypot(nx, ny, nz);
      nx /= len;
      ny /= len;

      const i = (y * size + x) * 4;
      data[i] = Math.round((nx * 0.5 + 0.5) * 255);
      data[i + 1] = Math.round((ny * 0.5 + 0.5) * 255);
      data[i + 2] = Math.round((nz / len * 0.5 + 0.5) * 255);
      data[i + 3] = 255;
    }
  }

  return data;
}

/**
 * Osso antigo: marfim manchado, poros, rachaduras e sujeira acumulada.
 */
export function createBoneTextures(size = 512) {
  const albedo = new Uint8Array(size * size * 4);
  const rough = new Uint8Array(size * size * 4);
  const height = new Float32Array(size * size);

  for (let y = 0; y < size; y += 1) {
    for (let x = 0; x < size; x += 1) {
      const u = x / size;
      const v = y / size;

      const mottle = fbm(u, v, 6, 5, 11);
      const grain = fbm(u, v, 48, 3, 27);
      const pores = Math.pow(fbm(u, v, 120, 2, 51), 3.2);
      const crack = Math.pow(ridged(u, v, 9, 4, 73), 14);
      const stain = Math.pow(fbm(u, v, 3, 4, 97), 2.1);

      // Marfim claro -> osso encardido
      const tone = clamp01(mottle * 0.7 + grain * 0.2 - stain * 0.3 + 0.34);
      let r = lerp(0.62, 0.97, tone);
      let g = lerp(0.57, 0.93, tone);
      let b = lerp(0.47, 0.84, tone);

      // Sujeira quente nos vãos
      const dirt = clamp01(stain * 0.62 + pores * 1.3);
      r = lerp(r, 0.34, dirt * 0.42);
      g = lerp(g, 0.27, dirt * 0.46);
      b = lerp(b, 0.2, dirt * 0.5);

      // Rachaduras escuras
      const dark = clamp01(crack * 1.3 + pores * 1.1);
      r *= 1 - dark * 0.5;
      g *= 1 - dark * 0.54;
      b *= 1 - dark * 0.58;

      const i = (y * size + x) * 4;
      albedo[i] = Math.round(clamp01(r) * 255);
      albedo[i + 1] = Math.round(clamp01(g) * 255);
      albedo[i + 2] = Math.round(clamp01(b) * 255);
      albedo[i + 3] = 255;

      const roughness = clamp01(0.46 + grain * 0.2 + dirt * 0.2 - tone * 0.1);
      rough[i] = 255;
      rough[i + 1] = Math.round(roughness * 255);
      rough[i + 2] = 0;
      rough[i + 3] = 255;

      height[y * size + x] = clamp01(tone * 0.55 + grain * 0.3 - crack * 1.2 - pores * 1.6 + 0.3);
    }
  }

  return {
    map: makeDataTexture(albedo, size, { colorSpace: THREE.SRGBColorSpace, repeat: 1 }),
    roughnessMap: makeDataTexture(rough, size),
    normalMap: makeDataTexture(heightToNormal(height, size, 2.6), size),
  };
}

/**
 * Chão do domínio: cinza queimada com fendas incandescentes.
 */
export function createGroundTextures(size = 512) {
  const albedo = new Uint8Array(size * size * 4);
  const emissive = new Uint8Array(size * size * 4);
  const height = new Float32Array(size * size);

  for (let y = 0; y < size; y += 1) {
    for (let x = 0; x < size; x += 1) {
      const u = x / size;
      const v = y / size;

      const base = fbm(u, v, 5, 5, 5);
      const ash = fbm(u, v, 30, 3, 19);
      const fissure = Math.pow(ridged(u, v, 4, 5, 41), 9);
      const web = Math.pow(ridged(u, v, 11, 3, 61), 12);
      const glow = clamp01(fissure * 1.15 + web * 0.5);

      let r = lerp(0.05, 0.15, base * 0.7 + ash * 0.3);
      let g = lerp(0.047, 0.142, base * 0.7 + ash * 0.3);
      let b = lerp(0.05, 0.145, base * 0.6 + ash * 0.3);

      r = lerp(r, 0.16, glow * 0.28);
      g = lerp(g, 0.055, glow * 0.24);
      b = lerp(b, 0.03, glow * 0.24);

      const i = (y * size + x) * 4;
      albedo[i] = Math.round(clamp01(r) * 255);
      albedo[i + 1] = Math.round(clamp01(g) * 255);
      albedo[i + 2] = Math.round(clamp01(b) * 255);
      albedo[i + 3] = 255;

      const heat = Math.pow(glow, 3.4);
      emissive[i] = Math.round(clamp01(heat * 1.15) * 255);
      emissive[i + 1] = Math.round(clamp01(heat * 0.24) * 255);
      emissive[i + 2] = Math.round(clamp01(heat * 0.08) * 255);
      emissive[i + 3] = 255;

      height[y * size + x] = clamp01(base * 0.6 + ash * 0.4 - glow * 1.4);
    }
  }

  return {
    map: makeDataTexture(albedo, size, { colorSpace: THREE.SRGBColorSpace }),
    emissiveMap: makeDataTexture(emissive, size, { colorSpace: THREE.SRGBColorSpace }),
    normalMap: makeDataTexture(heightToNormal(height, size, 1.7), size),
  };
}

/** Faísca redonda com queda suave — usada nas brasas e na cinza. */
export function createSparkTexture(size = 64) {
  const data = new Uint8Array(size * size * 4);
  const half = size / 2;

  for (let y = 0; y < size; y += 1) {
    for (let x = 0; x < size; x += 1) {
      const dx = (x + 0.5 - half) / half;
      const dy = (y + 0.5 - half) / half;
      const d = Math.hypot(dx, dy);
      const core = Math.pow(clamp01(1 - d), 2.6);
      const halo = Math.pow(clamp01(1 - d), 0.9) * 0.35;
      const a = clamp01(core + halo);

      const i = (y * size + x) * 4;
      data[i] = 255;
      data[i + 1] = Math.round(clamp01(core * 0.75 + 0.12) * 255);
      data[i + 2] = Math.round(clamp01(core * 0.34) * 255);
      data[i + 3] = Math.round(a * 255);
    }
  }

  const texture = new THREE.DataTexture(data, size, size, THREE.RGBAFormat);
  texture.colorSpace = THREE.SRGBColorSpace;
  texture.magFilter = THREE.LinearFilter;
  texture.minFilter = THREE.LinearFilter;
  texture.needsUpdate = true;
  return texture;
}

/** Lâmina do "Desmantelar": risco fino, brilhante no centro, some nas pontas. */
export function createSlashTexture(width = 256, heightPx = 64) {
  const data = new Uint8Array(width * heightPx * 4);

  for (let y = 0; y < heightPx; y += 1) {
    for (let x = 0; x < width; x += 1) {
      const u = x / (width - 1);
      const v = y / (heightPx - 1);

      const along = Math.pow(Math.sin(Math.PI * u), 1.4);
      const across = Math.pow(1 - Math.abs(v * 2 - 1), 5.5);
      const core = along * across;
      const bleed = along * Math.pow(1 - Math.abs(v * 2 - 1), 1.6) * 0.28;
      const a = clamp01(core + bleed);

      // Branco incandescente no miolo e apenas um resto quente na franja —
      // se a franja for vermelha pura, o corte vira um risco de laser.
      const i = (y * width + x) * 4;
      data[i] = 255;
      data[i + 1] = Math.round(clamp01(0.62 + core * 0.38) * 255);
      data[i + 2] = Math.round(clamp01(0.44 + core * 0.56) * 255);
      data[i + 3] = Math.round(a * 255);
    }
  }

  const texture = new THREE.DataTexture(data, width, heightPx, THREE.RGBAFormat);
  texture.colorSpace = THREE.SRGBColorSpace;
  texture.magFilter = THREE.LinearFilter;
  texture.minFilter = THREE.LinearFilter;
  texture.needsUpdate = true;
  return texture;
}
