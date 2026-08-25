/**
 * Primitivas ósseas procedurais.
 * Tudo é gerado por código: nenhum modelo externo é carregado.
 */
import * as THREE from 'three';
import { mergeGeometries } from 'three/examples/jsm/utils/BufferGeometryUtils.js';

const UP = new THREE.Vector3(0, 1, 0);
const ATTRS = ['position', 'normal', 'uv'];

const clamp01 = (v) => (v < 0 ? 0 : v > 1 ? 1 : v);
const lerp = (a, b, t) => a + (b - a) * t;

/**
 * Acumula geometrias já transformadas e funde tudo em poucas malhas.
 * Mantém o santuário inteiro em ~4 draw calls.
 */
export class GeoBatch {
  constructor() {
    this.parts = new Map();
  }

  add(key, geometry, matrix, uvScale) {
    if (matrix) geometry.applyMatrix4(matrix);
    normalizeGeometry(geometry);
    if (uvScale && uvScale !== 1) scaleUV(geometry, uvScale);

    if (!this.parts.has(key)) this.parts.set(key, []);
    this.parts.get(key).push(geometry);
    return this;
  }

  /**
   * Copia outro lote aplicando uma transformação global.
   * `uvScale` compensa o esticamento da textura em peças ampliadas.
   */
  merge(other, matrix, uvScale) {
    other.parts.forEach((list, key) => {
      list.forEach((geometry) => {
        const clone = geometry.clone();
        if (matrix) clone.applyMatrix4(matrix);
        if (uvScale && uvScale !== 1) scaleUV(clone, uvScale);
        if (!this.parts.has(key)) this.parts.set(key, []);
        this.parts.get(key).push(clone);
      });
    });
    return this;
  }

  count() {
    let total = 0;
    this.parts.forEach((list) => list.forEach((g) => {
      total += g.getAttribute('position').count;
    }));
    return total;
  }

  build(materials, { castShadow = true, receiveShadow = true } = {}) {
    const group = new THREE.Group();

    this.parts.forEach((list, key) => {
      const material = materials[key] ?? materials.bone;
      if (!material || list.length === 0) return;

      const merged = list.length === 1 ? list[0] : mergeGeometries(list, false);
      if (!merged) return;

      const mesh = new THREE.Mesh(merged, material);
      mesh.name = key;
      mesh.castShadow = castShadow && key !== 'void';
      mesh.receiveShadow = receiveShadow;
      group.add(mesh);
    });

    return group;
  }
}

function normalizeGeometry(geometry) {
  Object.keys(geometry.attributes).forEach((name) => {
    if (!ATTRS.includes(name)) geometry.deleteAttribute(name);
  });

  if (!geometry.getAttribute('normal')) geometry.computeVertexNormals();

  if (!geometry.getAttribute('uv')) {
    const count = geometry.getAttribute('position').count;
    geometry.setAttribute('uv', new THREE.BufferAttribute(new Float32Array(count * 2), 2));
  }

  if (!geometry.getIndex()) {
    const count = geometry.getAttribute('position').count;
    const array = count > 65535 ? new Uint32Array(count) : new Uint16Array(count);
    for (let i = 0; i < count; i += 1) array[i] = i;
    geometry.setIndex(new THREE.BufferAttribute(array, 1));
  }

  geometry.clearGroups();
  return geometry;
}

function scaleUV(geometry, factor) {
  const uv = geometry.getAttribute('uv');
  if (!uv) return;
  for (let i = 0; i < uv.count; i += 1) {
    uv.setXY(i, uv.getX(i) * factor, uv.getY(i) * factor);
  }
  uv.needsUpdate = true;
}

const scratch = new THREE.Object3D();

/** Monta uma matriz de transformação a partir de posição/rotação/escala. */
export function T({ pos, rot, quat, scale } = {}) {
  scratch.position.set(0, 0, 0);
  scratch.rotation.set(0, 0, 0);
  scratch.quaternion.identity();
  scratch.scale.set(1, 1, 1);

  if (pos) scratch.position.fromArray(pos);
  if (rot) scratch.rotation.set(rot[0], rot[1], rot[2]);
  if (quat) scratch.quaternion.copy(quat);
  if (typeof scale === 'number') scratch.scale.setScalar(scale);
  else if (scale) scratch.scale.fromArray(scale);

  scratch.updateMatrix();
  return scratch.matrix.clone();
}

/** Matriz que alinha o eixo +Y de uma peça entre dois pontos. */
export function span(from, to, { roll = 0, scale = 1 } = {}) {
  const a = Array.isArray(from) ? new THREE.Vector3().fromArray(from) : from;
  const b = Array.isArray(to) ? new THREE.Vector3().fromArray(to) : to;
  const dir = new THREE.Vector3().subVectors(b, a);
  const length = dir.length();
  const quat = new THREE.Quaternion().setFromUnitVectors(UP, dir.normalize());

  if (roll) {
    quat.multiply(new THREE.Quaternion().setFromAxisAngle(UP, roll));
  }

  const matrix = new THREE.Matrix4().compose(a, quat, new THREE.Vector3(scale, scale, scale));
  return { matrix, length, direction: dir };
}

/**
 * Tubo genérico ao longo de uma curva, com raio variável.
 * Base de ossos longos, costelas, chifres e vigas.
 */
export function tubeAlongCurve(curve, radiusAt, tubularSegments = 40, radialSegments = 10) {
  const frames = curve.computeFrenetFrames(tubularSegments, false);
  const positions = [];
  const normals = [];
  const uvs = [];
  const indices = [];
  const point = new THREE.Vector3();

  for (let i = 0; i <= tubularSegments; i += 1) {
    const t = i / tubularSegments;
    curve.getPointAt(t, point);
    const N = frames.normals[i];
    const B = frames.binormals[i];
    const radius = Math.max(radiusAt(t), 1e-5);

    for (let j = 0; j <= radialSegments; j += 1) {
      const v = (j / radialSegments) * Math.PI * 2;
      const sin = Math.sin(v);
      const cos = -Math.cos(v);

      const nx = cos * N.x + sin * B.x;
      const ny = cos * N.y + sin * B.y;
      const nz = cos * N.z + sin * B.z;

      positions.push(point.x + radius * nx, point.y + radius * ny, point.z + radius * nz);
      normals.push(nx, ny, nz);
      uvs.push(t * curve.getLength() * 0.08, j / radialSegments);
    }
  }

  const stride = radialSegments + 1;
  for (let i = 0; i < tubularSegments; i += 1) {
    for (let j = 0; j < radialSegments; j += 1) {
      const a = i * stride + j;
      const b = (i + 1) * stride + j;
      indices.push(a, b, a + 1, b, b + 1, a + 1);
    }
  }

  const geometry = new THREE.BufferGeometry();
  geometry.setAttribute('position', new THREE.Float32BufferAttribute(positions, 3));
  geometry.setAttribute('normal', new THREE.Float32BufferAttribute(normals, 3));
  geometry.setAttribute('uv', new THREE.Float32BufferAttribute(uvs, 2));
  geometry.setIndex(indices);
  geometry.computeVertexNormals();
  return geometry;
}

/** Perfil clássico de osso longo: extremidades bulbosas, diáfise fina. */
function boneRadius(t, { endR, midR, seed = 0, headScale = 1, footScale = 1, taper = 2.4 }) {
  const s = Math.abs(t * 2 - 1);
  const bulge = Math.pow(s, taper);
  const endScale = t > 0.5 ? headScale : footScale;
  let r = midR + (endR * endScale - midR) * bulge;
  r *= 1 + 0.055 * Math.sin(t * 12.7 + seed) + 0.03 * Math.sin(t * 29.3 + seed * 2.1);
  r *= Math.sqrt(Math.max(0, 1 - Math.pow(s, 18)));
  return r;
}

/** Osso longo reto (fêmur, úmero, viga do santuário). */
export function longBone(length, endR, midR, options = {}) {
  const curve = new THREE.LineCurve3(new THREE.Vector3(0, 0, 0), new THREE.Vector3(0, length, 0));
  const segments = options.segments ?? 30;
  const radialSegments = options.radialSegments ?? 12;
  return tubeAlongCurve(curve, (t) => boneRadius(t, { endR, midR, ...options }), segments, radialSegments);
}

/** Osso curvo — costelas, chifres, arcos do telhado. */
export function curvedBone(points, endR, midR, options = {}) {
  const curve = new THREE.CatmullRomCurve3(
    points.map((p) => (Array.isArray(p) ? new THREE.Vector3().fromArray(p) : p)),
  );
  const segments = options.segments ?? 44;
  const radialSegments = options.radialSegments ?? 10;
  return tubeAlongCurve(curve, (t) => boneRadius(t, { endR, midR, ...options }), segments, radialSegments);
}

/** Espinho / presa / chifre: cone orgânico levemente curvado. */
export function spike(length, baseR, options = {}) {
  const curve = new THREE.QuadraticBezierCurve3(
    new THREE.Vector3(0, 0, 0),
    new THREE.Vector3(options.bend ? options.bend * length * 0.3 : 0, length * 0.55, options.lean ? options.lean * length * 0.3 : 0),
    new THREE.Vector3(options.bend ? options.bend * length : 0, length, options.lean ? options.lean * length * 1.1 : 0),
  );
  const sharpness = options.sharpness ?? 1.7;
  return tubeAlongCurve(
    curve,
    (t) => Math.max(baseR * Math.pow(1 - t, sharpness) * (1 + 0.05 * Math.sin(t * 15)), 1e-4),
    options.segments ?? 18,
    options.radialSegments ?? 9,
  );
}

/** Esfera deformada por uma função — base do crânio e das articulações. */
export function deformedSphere(radius, widthSeg, heightSeg, deform) {
  const geometry = new THREE.SphereGeometry(radius, widthSeg, heightSeg);
  const position = geometry.getAttribute('position');
  const v = new THREE.Vector3();

  for (let i = 0; i < position.count; i += 1) {
    v.fromBufferAttribute(position, i);
    deform(v, i);
    position.setXYZ(i, v.x, v.y, v.z);
  }

  position.needsUpdate = true;
  geometry.computeVertexNormals();
  return geometry;
}

/**
 * Superfície paramétrica com espessura — telhado do santuário.
 * fn(u, v) -> Vector3, com u e v em [-1, 1].
 */
export function parametricSlab(fn, uSegments, vSegments, thickness) {
  const stride = uSegments + 1;
  const surface = [];
  const normals = [];

  for (let i = 0; i <= uSegments; i += 1) {
    for (let j = 0; j <= vSegments; j += 1) {
      const u = (i / uSegments) * 2 - 1;
      const v = (j / vSegments) * 2 - 1;
      surface.push(fn(u, v));
    }
  }

  const vStride = vSegments + 1;
  const at = (i, j) => surface[Math.min(Math.max(i, 0), uSegments) * vStride + Math.min(Math.max(j, 0), vSegments)];

  for (let i = 0; i <= uSegments; i += 1) {
    for (let j = 0; j <= vSegments; j += 1) {
      const du = new THREE.Vector3().subVectors(at(i + 1, j), at(i - 1, j));
      const dv = new THREE.Vector3().subVectors(at(i, j + 1), at(i, j - 1));
      const n = new THREE.Vector3().crossVectors(dv, du).normalize();
      if (n.y < 0) n.negate();
      normals.push(n);
    }
  }

  const positions = [];
  const normalOut = [];
  const uvs = [];
  const indices = [];
  const half = thickness * 0.5;

  const pushRing = (sign) => {
    const offset = positions.length / 3;

    for (let i = 0; i <= uSegments; i += 1) {
      for (let j = 0; j <= vSegments; j += 1) {
        const idx = i * vStride + j;
        const p = surface[idx];
        const n = normals[idx];
        positions.push(p.x + n.x * half * sign, p.y + n.y * half * sign, p.z + n.z * half * sign);
        normalOut.push(n.x * sign, n.y * sign, n.z * sign);
        uvs.push((i / uSegments) * 22, (j / vSegments) * 7);
      }
    }

    for (let i = 0; i < uSegments; i += 1) {
      for (let j = 0; j < vSegments; j += 1) {
        const a = offset + i * vStride + j;
        const b = offset + (i + 1) * vStride + j;
        if (sign > 0) indices.push(a, a + 1, b, a + 1, b + 1, b);
        else indices.push(a, b, a + 1, a + 1, b, b + 1);
      }
    }

    return offset;
  };

  const topOffset = pushRing(1);
  const bottomOffset = pushRing(-1);

  const border = [];
  for (let i = 0; i <= uSegments; i += 1) border.push(i * vStride + 0);
  for (let j = 1; j <= vSegments; j += 1) border.push(uSegments * vStride + j);
  for (let i = uSegments - 1; i >= 0; i -= 1) border.push(i * vStride + vSegments);
  for (let j = vSegments - 1; j >= 1; j -= 1) border.push(0 * vStride + j);

  for (let k = 0; k < border.length; k += 1) {
    const cur = border[k];
    const next = border[(k + 1) % border.length];
    const a = topOffset + cur;
    const b = topOffset + next;
    const c = bottomOffset + cur;
    const d = bottomOffset + next;
    indices.push(a, c, b, b, c, d);
  }

  const geometry = new THREE.BufferGeometry();
  geometry.setAttribute('position', new THREE.Float32BufferAttribute(positions, 3));
  geometry.setAttribute('normal', new THREE.Float32BufferAttribute(normalOut, 3));
  geometry.setAttribute('uv', new THREE.Float32BufferAttribute(uvs, 2));
  geometry.setIndex(indices);
  geometry.computeVertexNormals();
  return geometry;
}

/** Vértebra: corpo, processo espinhoso e processos transversos. */
function buildVertebra(batch, matrix, scale = 1, key = 'bone') {
  const local = new GeoBatch();

  local.add(key, new THREE.CylinderGeometry(0.42, 0.44, 0.34, 14, 1), T({ scale: [1.05, 1, 0.86] }));
  local.add(key, new THREE.TorusGeometry(0.3, 0.13, 8, 16, Math.PI * 1.25), T({ pos: [0, 0.02, -0.34], rot: [Math.PI / 2, 0, Math.PI / 2] }));
  local.add(key, spike(0.68, 0.15, { lean: -0.7, sharpness: 1.2 }), T({ pos: [0, 0.02, -0.5], rot: [Math.PI * 0.42, 0, 0] }));
  local.add(key, spike(0.42, 0.12, { sharpness: 1.1 }), T({ pos: [0.34, 0.04, -0.18], rot: [0, 0, -Math.PI * 0.42] }));
  local.add(key, spike(0.42, 0.12, { sharpness: 1.1 }), T({ pos: [-0.34, 0.04, -0.18], rot: [0, 0, Math.PI * 0.42] }));

  const composed = matrix.clone().multiply(T({ scale }));
  batch.merge(local, composed);
  return batch;
}

/** Coluna vertebral seguindo uma curva. */
export function buildSpine(batch, curve, count, radiusAt, key = 'bone') {
  const up = new THREE.Vector3(0, 1, 0);

  for (let i = 0; i < count; i += 1) {
    const t = count === 1 ? 0.5 : i / (count - 1);
    const point = curve.getPointAt(t);
    const tangent = curve.getTangentAt(t);
    const quat = new THREE.Quaternion().setFromUnitVectors(up, tangent.clone().normalize());
    const matrix = new THREE.Matrix4().compose(point, quat, new THREE.Vector3(1, 1, 1));
    buildVertebra(batch, matrix, radiusAt(t), key);
  }

  return batch;
}

/**
 * Crânio completo.
 *
 * A calota, as têmporas, a maxila e as órbitas saem de UMA única esfera
 * deformada — órbitas e abertura nasal são cavadas de verdade na malha, em vez
 * de peças coladas por cima. Isso evita z-fighting e dá um perfil contínuo.
 * Depois entram arcos zigomáticos, arcadas dentárias e a mandíbula articulada.
 *
 * Espaço local: +Z é a face, +Y é o topo, comprimento total ≈ 2 unidades.
 */
export function buildSkull({ jawOpen = 0.55, detail = 1, teeth = true } = {}) {
  const batch = new GeoBatch();
  const seg = detail > 0.6 ? 46 : 24;
  const segY = detail > 0.6 ? 32 : 16;

  const orbitDir = (side) => new THREE.Vector3(side * 0.52, 0.05, 0.84).normalize();
  const nasalDir = new THREE.Vector3(0, -0.34, 0.94).normalize();
  const orbits = [-1, 1].map(orbitDir);

  // --- Calota, têmporas e maxila em uma superfície contínua --------------
  const cranium = deformedSphere(1, seg, segY, (v) => {
    const n = v.clone().normalize();
    const front = clamp01(n.z);
    const back = clamp01(-n.z);
    const up = clamp01(n.y);
    const down = clamp01(-n.y);
    const flank = Math.abs(n.x);

    v.x *= 0.8;
    v.y *= 0.84;
    v.z *= 1.04;

    // Occipital saliente atrás
    v.z -= Math.pow(back, 2) * 0.18;
    // Testa reta e recuada
    v.z -= Math.pow(front, 1.5) * Math.pow(up, 1.3) * 0.24;
    v.y -= Math.pow(up, 3) * 0.05;
    // Fossa temporal: os lados afundam atrás das órbitas
    v.x *= 1 - Math.pow(flank, 1.8) * 0.18;
    // Maxila projetada para baixo e para a frente
    const muzzle = Math.pow(front, 1.3) * Math.pow(down, 0.8);
    v.z += muzzle * 0.36;
    v.y -= muzzle * 0.3;
    v.x *= 1 - muzzle * 0.32;

    // Órbitas cavadas na frente do crânio
    orbits.forEach((dir) => {
      const k = Math.exp(-Math.pow(n.distanceTo(dir) / 0.44, 2));
      v.addScaledVector(n, -k * 0.34);
    });

    // Abertura nasal, entre e abaixo das órbitas
    const nasal = Math.exp(-Math.pow(n.distanceTo(nasalDir) / 0.16, 2));
    v.addScaledVector(n, -nasal * 0.3);
  });
  batch.add('bone', cranium);

  // --- Escuridão no fundo das órbitas e do nariz -------------------------
  const socketMatrix = (dir, distance, scale) => {
    const quat = new THREE.Quaternion().setFromUnitVectors(UP, dir.clone().negate());
    return new THREE.Matrix4().compose(
      dir.clone().multiplyScalar(distance),
      quat,
      new THREE.Vector3(scale, scale, scale),
    );
  };

  orbits.forEach((dir) => {
    batch.add('void', new THREE.ConeGeometry(0.3, 0.62, 22, 1, true), socketMatrix(dir, 0.42, 1));
    batch.add('void', new THREE.SphereGeometry(0.29, 18, 12), T({ pos: dir.clone().multiplyScalar(0.4).toArray() }));
  });

  batch.add('void', new THREE.ConeGeometry(0.13, 0.42, 14, 1, true), socketMatrix(nasalDir, 0.58, 1));
  batch.add('void', new THREE.SphereGeometry(0.12, 14, 10), T({ pos: nasalDir.clone().multiplyScalar(0.6).toArray(), scale: [1, 1.35, 1] }));

  // --- Arcada superciliar, crista nasal e zigomáticos --------------------
  [-1, 1].forEach((side) => {
    batch.add(
      'bone',
      curvedBone(
        [
          [side * 0.05, 0.24, 0.66],
          [side * 0.34, 0.3, 0.63],
          [side * 0.6, 0.2, 0.42],
          [side * 0.72, 0.02, 0.06],
        ],
        0.13,
        0.1,
        { segments: 22, radialSegments: 8 },
      ),
    );

    // Borda inferior da órbita
    batch.add(
      'bone',
      curvedBone(
        [
          [side * 0.14, -0.18, 0.7],
          [side * 0.38, -0.22, 0.64],
          [side * 0.56, -0.16, 0.44],
        ],
        0.055,
        0.042,
        { segments: 16, radialSegments: 7 },
      ),
    );

    // Arco zigomático indo da face até a têmpora
    batch.add(
      'bone',
      curvedBone(
        [
          [side * 0.56, -0.24, 0.38],
          [side * 0.7, -0.18, 0.12],
          [side * 0.73, -0.08, -0.2],
          [side * 0.58, 0.02, -0.5],
        ],
        0.06,
        0.045,
        { segments: 24, radialSegments: 8 },
      ),
    );
  });

  // Crista entre as órbitas (glabela → espinha nasal)
  batch.add(
    'bone',
    curvedBone([[0, 0.2, 0.66], [0, 0.02, 0.72], [0, -0.16, 0.74]], 0.075, 0.055, {
      segments: 14,
      radialSegments: 7,
    }),
  );

  // --- Suturas ----------------------------------------------------------
  if (detail > 0.6) {
    batch.add(
      'seam',
      curvedBone([[0, 0.78, 0.5], [0, 0.86, 0.02], [0, 0.76, -0.48], [0, 0.44, -0.86]], 0.02, 0.02, {
        segments: 26,
        radialSegments: 6,
      }),
    );
    batch.add(
      'seam',
      curvedBone([[-0.66, 0.34, -0.02], [-0.34, 0.8, 0.08], [0.34, 0.8, 0.08], [0.66, 0.34, -0.02]], 0.018, 0.018, {
        segments: 26,
        radialSegments: 6,
      }),
    );
  }

  batch.add('seam', new THREE.SphereGeometry(0.24, 14, 10), T({ pos: [0, -0.58, -0.34], scale: [1, 0.5, 1.15] }));

  // --- Arcadas dentárias ------------------------------------------------
  // Elipse com o "sorriso" caindo para trás: os molares ficam mais baixos
  // que os incisivos, como numa arcada de verdade.
  const archPoint = (k, rx, rz, front, yFront, yBack) => {
    const angle = k * Math.PI;
    const openness = Math.sin(angle);
    return new THREE.Vector3(
      -Math.cos(angle) * rx,
      lerp(yBack, yFront, openness),
      openness * rz + front,
    );
  };

  const toothCount = detail > 0.6 ? 13 : 9;

  const addArch = (target, rx, rz, front, yFront, yBack, dir) => {
    for (let i = 0; i < toothCount; i += 1) {
      const k = i / (toothCount - 1);
      const p = archPoint(k, rx, rz, front, yFront, yBack);
      const centrality = 1 - Math.abs(k - 0.5) * 2;
      const canine = Math.exp(-Math.pow((Math.abs(k - 0.5) - 0.26) * 9, 2));
      const size = 0.16 + centrality * 0.05 + canine * 0.16;
      const outward = Math.atan2(p.x, p.z - front);

      target.add(
        'tooth',
        spike(size * 1.6, size * 0.46, { sharpness: 1.5 }),
        T({ pos: p.toArray(), rot: [dir > 0 ? 0 : Math.PI, 0, 0] }).multiply(
          T({ rot: [0.1, -outward * 0.25, 0] }),
        ),
      );
    }
  };

  const maxillaArch = (k) => archPoint(k, 0.38, 0.56, 0.42, -0.72, -0.9);
  batch.add(
    'bone',
    tubeAlongCurve(
      new THREE.CatmullRomCurve3(Array.from({ length: 13 }, (_, i) => maxillaArch(i / 12))),
      (t) => 0.13 - Math.abs(t - 0.5) * 0.05,
      28,
      9,
    ),
  );
  if (teeth) addArch(batch, 0.38, 0.56, 0.42, -0.76, -0.94, -1);

  // --- Mandíbula (articulada nos côndilos) ------------------------------
  const jaw = new GeoBatch();
  const mandibleArch = (k) => archPoint(k, 0.4, 0.55, 0.4, -0.92, -1.1);

  jaw.add(
    'bone',
    tubeAlongCurve(
      new THREE.CatmullRomCurve3(Array.from({ length: 13 }, (_, i) => mandibleArch(i / 12))),
      (t) => 0.16 - Math.abs(t - 0.5) * 0.05,
      28,
      10,
    ),
  );
  if (teeth) addArch(jaw, 0.4, 0.55, 0.4, -0.88, -1.06, 1);

  [-1, 1].forEach((side) => {
    // Ramo ascendente, do ângulo da mandíbula até o côndilo
    jaw.add(
      'bone',
      curvedBone(
        [
          [side * 0.4, -1.1, 0.34],
          [side * 0.48, -1.06, -0.06],
          [side * 0.58, -0.68, -0.28],
          [side * 0.58, -0.3, -0.22],
        ],
        0.16,
        0.12,
        { segments: 22, radialSegments: 9 },
      ),
    );
    jaw.add('bone', new THREE.SphereGeometry(0.14, 12, 9), T({ pos: [side * 0.58, -0.28, -0.2], scale: [1.35, 0.8, 0.9] }));
  });

  const hinge = new THREE.Vector3(0, -0.28, -0.2);
  const jawMatrix = new THREE.Matrix4()
    .makeTranslation(hinge.x, hinge.y, hinge.z)
    .multiply(new THREE.Matrix4().makeRotationX(jawOpen))
    .multiply(new THREE.Matrix4().makeTranslation(-hinge.x, -hinge.y, -hinge.z));

  batch.merge(jaw, jawMatrix);

  // Goela: escuridão que preenche o vão entre as arcadas
  batch.add('void', new THREE.SphereGeometry(0.4, 18, 12), T({ pos: [0, -0.78, 0.26], scale: [0.95, 0.7, 1.05] }));

  return batch;
}

/** Mão esquelética com falanges, pronta para se apoiar no chão. */
export function buildHand({ spread = 1, curl = 0.34 } = {}) {
  const batch = new GeoBatch();

  // Carpo + metacarpo
  batch.add('bone', deformedSphere(1, 20, 14, (v) => {
    const n = v.clone().normalize();
    v.x *= 0.72;
    v.y *= 0.32;
    v.z *= 0.8;
    v.z += Math.pow(clamp01(n.z), 2) * 0.1;
  }), T({ pos: [0, 0, 0.1] }));

  batch.add('bone', new THREE.SphereGeometry(0.34, 16, 12), T({ pos: [0, 0.02, -0.5], scale: [1.1, 0.72, 0.9] }));

  const fingers = [
    { angle: 0.92, length: [0.5, 0.4, 0.3], base: [0.5, -0.02, 0.18], lift: 0.1, thumb: true },
    { angle: 0.3, length: [0.72, 0.5, 0.34], base: [0.42, 0, 0.6] },
    { angle: 0.08, length: [0.82, 0.56, 0.36], base: [0.16, 0.02, 0.72] },
    { angle: -0.14, length: [0.78, 0.54, 0.36], base: [-0.12, 0.02, 0.7] },
    { angle: -0.42, length: [0.62, 0.44, 0.3], base: [-0.38, -0.02, 0.56] },
  ];

  fingers.forEach((finger, index) => {
    const angle = finger.angle * spread;
    let x = finger.base[0];
    let y = finger.base[1];
    let z = finger.base[2];
    let pitch = finger.thumb ? 0.1 : 0.06;

    batch.add('bone', new THREE.SphereGeometry(0.21, 12, 9), T({ pos: [x, y, z], scale: [1, 0.8, 1] }));

    finger.length.forEach((len, joint) => {
      pitch += curl * (joint === 0 ? 0.5 : 1) * (finger.thumb ? 0.7 : 1);
      const dx = Math.sin(angle) * len * Math.cos(pitch);
      const dz = Math.cos(angle) * len * Math.cos(pitch);
      const dy = -Math.sin(pitch) * len;

      const from = new THREE.Vector3(x, y, z);
      const to = new THREE.Vector3(x + dx, Math.max(y + dy, 0.08), z + dz);
      const { matrix, length } = span(from, to);
      const radius = 0.165 - joint * 0.025 - index * 0.005;

      batch.add('bone', longBone(length, radius * 1.35, radius, { segments: 12, radialSegments: 9, seed: index + joint }), matrix);
      batch.add('bone', new THREE.SphereGeometry(radius * 1.25, 10, 8), T({ pos: to.toArray(), scale: [1, 0.85, 1] }));

      x = to.x;
      y = to.y;
      z = to.z;
    });
  });

  return batch;
}

/**
 * Braço colossal: escápula, úmero, rádio, ulna e mão.
 * Recebe os pontos-chave em espaço de mundo.
 */
export function buildArm(batch, { shoulder, elbow, wrist, hand, side = 1, thickness = 1 }) {
  const S = new THREE.Vector3().fromArray(shoulder);
  const E = new THREE.Vector3().fromArray(elbow);
  const W = new THREE.Vector3().fromArray(wrist);

  // Escápula / ombro
  batch.add('bone', deformedSphere(1, 22, 16, (v) => {
    const n = v.clone().normalize();
    v.x *= 2.6;
    v.y *= 2.3;
    v.z *= 1.1;
    v.z += Math.pow(clamp01(-n.z), 2) * 0.8;
  }), T({ pos: shoulder, rot: [0.2, side * -0.5, side * 0.5], scale: thickness }));

  batch.add('bone', new THREE.SphereGeometry(1.5 * thickness, 18, 14), T({ pos: shoulder }));

  // Úmero
  const humerus = span(S, E);
  batch.add(
    'bone',
    longBone(humerus.length, 1.45 * thickness, 0.92 * thickness, { segments: 30, radialSegments: 14, seed: side * 3 }),
    humerus.matrix,
  );
  batch.add('bone', new THREE.SphereGeometry(1.35 * thickness, 18, 14), T({ pos: elbow }));

  // Rádio + ulna
  const forearmDir = new THREE.Vector3().subVectors(W, E).normalize();
  const offsetAxis = new THREE.Vector3().crossVectors(forearmDir, UP).normalize().multiplyScalar(0.72 * thickness);

  [1, -1].forEach((sign, i) => {
    const from = E.clone().addScaledVector(offsetAxis, sign * 0.55);
    const to = W.clone().addScaledVector(offsetAxis, sign * 0.42);
    const bone = span(from, to);
    batch.add(
      'bone',
      longBone(bone.length, (i === 0 ? 1.05 : 0.9) * thickness, (i === 0 ? 0.62 : 0.52) * thickness, {
        segments: 28,
        radialSegments: 12,
        seed: side * 7 + i,
      }),
      bone.matrix,
    );
  });

  batch.add('bone', new THREE.SphereGeometry(1.05 * thickness, 16, 12), T({ pos: wrist }));

  // Mão apoiada no solo
  const handBatch = buildHand({ spread: hand.spread ?? 1.05, curl: hand.curl ?? 0.3 });
  const handScale = hand.scale ?? 4.6 * thickness;
  const handMatrix = new THREE.Matrix4().compose(
    new THREE.Vector3().fromArray(hand.pos),
    new THREE.Quaternion().setFromEuler(new THREE.Euler(hand.pitch ?? 0.14, hand.yaw ?? 0, hand.roll ?? 0)),
    new THREE.Vector3(handScale, handScale, handScale),
  );
  batch.merge(handBatch, handMatrix);

  // Punho ligando antebraço e mão
  const wristLink = span(W, new THREE.Vector3().fromArray(hand.pos));
  batch.add(
    'bone',
    longBone(wristLink.length, 1.0 * thickness, 0.8 * thickness, { segments: 14, radialSegments: 10 }),
    wristLink.matrix,
  );

  return batch;
}
