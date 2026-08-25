/**
 * Montagem do 伏魔御廚子 — Malevolent Shrine.
 *
 * Anatomia da estrutura, de baixo para cima:
 *   1. plataforma de pedra queimada e monte de crânios
 *   2. colunata de fêmures colossais e colunas vertebrais
 *   3. paredes de costelas entre os pilares
 *   4. telhado inferior em duas águas, com caibros de osso e beirais curvos
 *   5. segundo pavimento e telhado superior
 *   6. cumeeira vertebral coroada de chifres
 *   7. crânio gigante de boca aberta na frente
 *   8. dois braços descarnados apoiados no chão
 */
import * as THREE from 'three';
import {
  GeoBatch,
  T,
  span,
  longBone,
  curvedBone,
  spike,
  deformedSphere,
  parametricSlab,
  tubeAlongCurve,
  buildSpine,
  buildSkull,
  buildArm,
} from './bones.js';

const TAU = Math.PI * 2;

/** PRNG determinístico: o santuário é sempre o mesmo em qualquer máquina. */
function makeRandom(seed = 20180617) {
  let state = seed >>> 0;
  return () => {
    state ^= state << 13;
    state ^= state >>> 17;
    state ^= state << 5;
    state >>>= 0;
    return state / 4294967296;
  };
}

export const SHRINE_METRICS = {
  platformTop: 8.5,
  eaveY: 34,
  ridgeY: 48,
  upperEaveY: 52,
  upperRidgeY: 62,
  crownY: 72,
  skullCenter: new THREE.Vector3(0, 22, 50),
  skullScale: 15,
};

/** Perfil de telhado japonês: concavidade central e cantos empinados. */
function roofSurface({ ridgeY, eaveY, ridgeHalfLen, eaveHalfLen, halfDepth, cornerLift, sag = 1.75 }) {
  return (u, v) => {
    const a = Math.abs(v);
    const drop = Math.pow(a, sag);
    const corner = Math.pow(Math.abs(u), 9.0);

    const y = ridgeY + (eaveY - ridgeY) * drop + cornerLift * corner * Math.pow(a, 2.4);
    const halfLen = ridgeHalfLen + (eaveHalfLen - ridgeHalfLen) * a;
    const x = u * halfLen;
    const z = v * halfDepth * (1 + 0.16 * corner);

    return new THREE.Vector3(x, y, z);
  };
}

function buildRoof(batch, config, { rafters = 22, quality = 1 }) {
  const surface = roofSurface(config);
  const uSeg = Math.round(46 * quality);
  const vSeg = Math.round(20 * quality);

  batch.add('roof', parametricSlab(surface, uSeg, vSeg, config.thickness ?? 1.7));

  // Caibros: nervuras de osso descendo da cumeeira até o beiral
  const rafterCount = Math.round(rafters * quality);
  for (let i = 0; i <= rafterCount; i += 1) {
    const u = (i / rafterCount) * 2 - 1;

    [-1, 1].forEach((sideV) => {
      const points = [];
      for (let k = 0; k <= 6; k += 1) {
        const v = (k / 6) * sideV;
        const p = surface(u, v);
        p.y += (config.thickness ?? 1.7) * 0.5 + 0.32;
        points.push(p);
      }
      batch.add(
        'bone',
        curvedBone(points, 0.5, 0.34, { segments: Math.round(24 * quality), radialSegments: 7 }),
      );
    });
  }

  // Viga do beiral, seguindo o contorno empinado
  [-1, 1].forEach((sideV) => {
    const points = [];
    for (let k = 0; k <= 14; k += 1) {
      const u = (k / 14) * 2 - 1;
      const p = surface(u, sideV);
      p.y -= 0.2;
      points.push(p);
    }
    batch.add(
      'bone',
      curvedBone(points, 1.25, 0.95, { segments: Math.round(52 * quality), radialSegments: 10 }),
    );
  });

  // Chifres nas quatro pontas do beiral
  [-1, 1].forEach((sideU) => {
    [-1, 1].forEach((sideV) => {
      const tip = surface(sideU, sideV);
      batch.add(
        'bone',
        spike(config.hornLength ?? 13, 1.15, { sharpness: 1.9, bend: sideU * 0.28, lean: sideV * 0.34 }),
        T({ pos: tip.toArray(), rot: [sideV * -0.55, 0, sideU * 0.5] }),
      );
    });
  });

  return surface;
}

/** Cumeeira: coluna vertebral deitada, coroada de espinhos. */
function buildRidge(batch, { ridgeY, ridgeHalfLen, vertebrae = 16, spikeHeight = 5.5, quality = 1 }) {
  const curve = new THREE.CatmullRomCurve3([
    new THREE.Vector3(-ridgeHalfLen, ridgeY + 0.6, 0),
    new THREE.Vector3(-ridgeHalfLen * 0.4, ridgeY + 1.5, 0),
    new THREE.Vector3(ridgeHalfLen * 0.4, ridgeY + 1.5, 0),
    new THREE.Vector3(ridgeHalfLen, ridgeY + 0.6, 0),
  ]);

  batch.add('bone', tubeAlongCurve(curve, () => 1.5, Math.round(40 * quality), 12));

  const count = Math.round(vertebrae * quality);
  for (let i = 0; i < count; i += 1) {
    const t = (i + 0.5) / count;
    const p = curve.getPointAt(t);
    const scale = 1.6 + Math.sin(t * Math.PI) * 0.7;

    batch.add('bone', new THREE.SphereGeometry(1.5 * scale * 0.6, 12, 9), T({ pos: p.toArray(), scale: [1, 0.8, 1.1] }));
    batch.add(
      'bone',
      spike(spikeHeight * (0.7 + Math.sin(t * Math.PI) * 0.5), 0.7, { sharpness: 1.6, lean: -0.18 }),
      T({ pos: [p.x, p.y + 1, p.z], rot: [-0.14, 0, 0] }),
    );
  }
}

/** Um crânio genérico, reaproveitado no monte da base e nos ornamentos. */
function skullCache() {
  const cache = new Map();
  return (detail) => {
    const key = detail > 0.6 ? 'high' : 'low';
    if (!cache.has(key)) cache.set(key, buildSkull({ jawOpen: key === 'high' ? 0.5 : 0.34, detail }));
    return cache.get(key);
  };
}

function buildPlatform(batch, random, quality) {
  const getSkull = skullCache();

  // Degraus de pedra
  const tiers = [
    { w: 104, d: 68, y0: -1, y1: 2.6 },
    { w: 92, d: 59, y0: 2.6, y1: 5.6 },
    { w: 80, d: 50, y0: 5.6, y1: 8.5 },
  ];

  tiers.forEach((tier, index) => {
    const height = tier.y1 - tier.y0;
    const geometry = new THREE.BoxGeometry(tier.w, height, tier.d, 4, 1, 4);
    batch.add('stone', geometry, T({ pos: [0, (tier.y0 + tier.y1) / 2, 0] }), 7);

    // Frisos de osso na borda de cada degrau
    const beam = index < 2 ? 0.9 : 0.7;
    [-1, 1].forEach((sz) => {
      batch.add(
        'bone',
        longBone(tier.w, beam * 1.3, beam, { segments: 16, radialSegments: 9 }),
        T({ pos: [-tier.w / 2, tier.y1 - 0.2, (sz * tier.d) / 2], rot: [0, 0, -Math.PI / 2] }),
      );
    });
    [-1, 1].forEach((sx) => {
      batch.add(
        'bone',
        longBone(tier.d, beam * 1.3, beam, { segments: 16, radialSegments: 9 }),
        T({ pos: [(sx * tier.w) / 2, tier.y1 - 0.2, -tier.d / 2], rot: [Math.PI / 2, 0, 0] }),
      );
    });
  });

  // Entulho sobre os degraus: crânios e ossos quebrando as lajes limpas
  const rubbleCount = Math.round(38 * quality);
  for (let i = 0; i < rubbleCount; i += 1) {
    const tier = tiers[Math.floor(random() * 3)];
    const edge = random() < 0.62;
    const x = (random() * 2 - 1) * (tier.w / 2 - 3);
    const z = edge
      ? Math.sign(random() - 0.5) * (tier.d / 2 - 2 - random() * 5)
      : (random() * 2 - 1) * (tier.d / 2 - 3);
    const scale = 1.4 + random() * 2.2;

    batch.merge(
      getSkull(0.3),
      T({
        pos: [x, tier.y1 + scale * 0.35, z],
        rot: [random() * 0.8 - 0.4, random() * TAU, random() * 0.8 - 0.4],
        scale,
      }),
      scale * 0.8,
    );
  }

  for (let i = 0; i < rubbleCount; i += 1) {
    const tier = tiers[Math.floor(random() * 3)];
    const length = 3 + random() * 8;
    batch.add(
      'bone',
      longBone(length, 0.45 + random() * 0.3, 0.25 + random() * 0.18, {
        segments: 10,
        radialSegments: 8,
        seed: i + 300,
      }),
      T({
        pos: [
          (random() * 2 - 1) * (tier.w / 2 - 2),
          tier.y1 + 0.5,
          (random() * 2 - 1) * (tier.d / 2 - 2),
        ],
        rot: [Math.PI / 2 + (random() - 0.5) * 0.5, random() * TAU, (random() - 0.5) * 0.4],
      }),
    );
  }

  // Monte de crânios em volta da plataforma
  const skullCount = Math.round(46 * quality);
  for (let i = 0; i < skullCount; i += 1) {
    const angle = random() * TAU;
    const ring = 40 + random() * 24;
    const x = Math.cos(angle) * ring * 1.35;
    const z = Math.sin(angle) * ring;
    const scale = 2.6 + random() * 3.4;
    const y = 0.4 + random() * 1.6;

    batch.merge(
      getSkull(1),
      T({
        pos: [x, y + scale * 0.4, z],
        rot: [random() * 0.9 - 0.45, random() * TAU, random() * 0.9 - 0.45],
        scale,
      }),
      scale * 0.7,
    );
  }

  // Ossos soltos no entulho
  const boneCount = Math.round(90 * quality);
  for (let i = 0; i < boneCount; i += 1) {
    const angle = random() * TAU;
    const ring = 36 + random() * 32;
    const length = 5 + random() * 14;

    batch.add(
      'bone',
      longBone(length, 0.55 + random() * 0.4, 0.3 + random() * 0.25, { segments: 12, radialSegments: 8, seed: i }),
      T({
        pos: [Math.cos(angle) * ring * 1.35, 0.4 + random() * 1.4, Math.sin(angle) * ring],
        rot: [Math.PI / 2 + (random() - 0.5) * 0.6, random() * TAU, (random() - 0.5) * 0.5],
      }),
    );
  }
}

function buildColonnade(batch, quality) {
  const getSkull = skullCache();
  const top = SHRINE_METRICS.eaveY - 1.5;
  const bottom = SHRINE_METRICS.platformTop - 0.4;

  const columns = [];
  [-1, 1].forEach((sz) => {
    [-32, -11, 11, 32].forEach((x) => {
      columns.push({ x, z: sz * 18, corner: Math.abs(x) > 20 });
    });
  });

  columns.forEach((column, index) => {
    if (column.corner) {
      // Pilar-fêmur colossal
      const bone = span([column.x, bottom, column.z], [column.x, top, column.z]);
      batch.add(
        'bone',
        longBone(bone.length, 3.4, 2.3, { segments: Math.round(34 * quality), radialSegments: 16, seed: index }),
        bone.matrix,
      );
      batch.add('bone', new THREE.SphereGeometry(3.1, 16, 12), T({ pos: [column.x, top, column.z], scale: [1, 0.8, 1] }));
    } else {
      // Coluna vertebral
      const curve = new THREE.CatmullRomCurve3([
        new THREE.Vector3(column.x, bottom, column.z),
        new THREE.Vector3(column.x + Math.sign(column.x) * 0.8, (bottom + top) / 2, column.z * 0.94),
        new THREE.Vector3(column.x, top, column.z),
      ]);
      buildSpine(batch, curve, Math.round(15 * quality), () => 3.4);
      batch.add('bone', tubeAlongCurve(curve, () => 1.5, Math.round(24 * quality), 10));
    }

    // Crânio no capitel
    batch.merge(
      getSkull(0.3),
      T({ pos: [column.x, top + 2.6, column.z], rot: [0, column.z > 0 ? 0 : Math.PI, 0], scale: 2.5 }),
      2,
    );
  });

  // Arquitraves ligando os capitéis
  [-1, 1].forEach((sz) => {
    [top + 0.6, SHRINE_METRICS.platformTop + 8].forEach((y) => {
      batch.add(
        'bone',
        longBone(68, 1.9, 1.4, { segments: Math.round(30 * quality), radialSegments: 12 }),
        T({ pos: [-34, y, sz * 18], rot: [0, 0, -Math.PI / 2] }),
      );
    });
  });
  [-1, 1].forEach((sx) => {
    batch.add(
      'bone',
      longBone(38, 1.9, 1.4, { segments: Math.round(22 * quality), radialSegments: 12 }),
      T({ pos: [sx * 32, top + 0.6, -19], rot: [Math.PI / 2, 0, 0] }),
    );
  });
}

/** Paredes de costelas entre os pilares. */
function buildRibWalls(batch, quality) {
  const bottom = SHRINE_METRICS.platformTop;
  const top = SHRINE_METRICS.eaveY - 3;

  // Fundo
  const backCount = Math.round(17 * quality);
  for (let i = 0; i < backCount; i += 1) {
    const t = i / (backCount - 1);
    const x = (t * 2 - 1) * 33;
    const bow = 1 - Math.abs(t * 2 - 1) * 0.55;

    batch.add(
      'bone',
      curvedBone(
        [
          [x, bottom, -20],
          [x * 1.04, bottom + (top - bottom) * 0.34, -20 - bow * 3.4],
          [x * 1.02, bottom + (top - bottom) * 0.72, -20 - bow * 2.6],
          [x * 0.95, top, -19],
        ],
        0.95,
        0.66,
        { segments: Math.round(26 * quality), radialSegments: 8, seed: i },
      ),
    );
  }

  // Laterais
  [-1, 1].forEach((sx) => {
    const sideCount = Math.round(13 * quality);
    for (let i = 0; i < sideCount; i += 1) {
      const t = i / (sideCount - 1);
      const z = (t * 2 - 1) * 17;
      const bow = 1 - Math.abs(t * 2 - 1) * 0.5;

      batch.add(
        'bone',
        curvedBone(
          [
            [sx * 32, bottom, z],
            [sx * (32 + bow * 3.6), bottom + (top - bottom) * 0.36, z * 1.02],
            [sx * (32 + bow * 2.8), bottom + (top - bottom) * 0.74, z * 1.0],
            [sx * 31, top, z * 0.96],
          ],
          0.9,
          0.6,
          { segments: Math.round(24 * quality), radialSegments: 8, seed: i + 40 },
        ),
      );
    }
  });

  // Grade de costelas curtas na frente, emoldurando o crânio
  [-1, 1].forEach((sx) => {
    const count = Math.round(6 * quality);
    for (let i = 0; i < count; i += 1) {
      const t = i / Math.max(count - 1, 1);
      const x = sx * (14 + t * 18);

      batch.add(
        'bone',
        curvedBone(
          [
            [x, bottom + 2, 18],
            [x * 1.02, bottom + 10, 20.5],
            [x * 0.98, bottom + 18, 20],
            [x * 0.9, top, 18],
          ],
          0.8,
          0.55,
          { segments: Math.round(20 * quality), radialSegments: 8, seed: i + 80 },
        ),
      );
    }
  });
}

/** Segundo pavimento entre os dois telhados. */
function buildUpperStory(batch, quality) {
  const bottom = SHRINE_METRICS.ridgeY - 6;
  const top = SHRINE_METRICS.upperEaveY - 1;

  [-1, 1].forEach((sz) => {
    [-19, -6.5, 6.5, 19].forEach((x) => {
      const bone = span([x, bottom, sz * 9], [x, top, sz * 9]);
      batch.add(
        'bone',
        longBone(bone.length, 1.5, 1.05, { segments: Math.round(16 * quality), radialSegments: 10 }),
        bone.matrix,
      );
    });
  });

  [-1, 1].forEach((sz) => {
    batch.add(
      'bone',
      longBone(42, 1.2, 0.9, { segments: 16, radialSegments: 9 }),
      T({ pos: [-21, top, sz * 9], rot: [0, 0, -Math.PI / 2] }),
    );
  });

  const count = Math.round(11 * quality);
  for (let i = 0; i < count; i += 1) {
    const t = i / (count - 1);
    const x = (t * 2 - 1) * 20;
    batch.add(
      'bone',
      curvedBone(
        [
          [x, bottom, -9],
          [x, bottom + (top - bottom) * 0.5, -10.6],
          [x, top, -8.6],
        ],
        0.6,
        0.42,
        { segments: 16, radialSegments: 7, seed: i + 120 },
      ),
    );
  }
}

/** Coluna vertebral colossal subindo pelas costas do santuário. */
function buildBackbone(batch, quality) {
  const curve = new THREE.CatmullRomCurve3([
    new THREE.Vector3(0, 4, -34),
    new THREE.Vector3(0, 18, -32),
    new THREE.Vector3(0, 34, -28),
    new THREE.Vector3(0, 50, -22),
    new THREE.Vector3(0, 62, -14),
    new THREE.Vector3(0, SHRINE_METRICS.crownY, -4),
  ]);

  batch.add('bone', tubeAlongCurve(curve, (t) => 2.4 - t * 1.0, Math.round(60 * quality), 12));
  buildSpine(batch, curve, Math.round(24 * quality), (t) => 3.6 - t * 1.4);

  // Costelas gigantes abraçando a estrutura
  const ribCount = Math.round(9 * quality);
  for (let i = 0; i < ribCount; i += 1) {
    const t = 0.08 + (i / ribCount) * 0.62;
    const origin = curve.getPointAt(t);
    const reach = 34 - t * 12;
    const drop = 10 + t * 6;

    [-1, 1].forEach((sx) => {
      batch.add(
        'bone',
        curvedBone(
          [
            [origin.x + sx * 2, origin.y, origin.z],
            [sx * reach * 0.5, origin.y + 2.4, origin.z + 10],
            [sx * reach * 0.86, origin.y - drop * 0.3, origin.z + 24],
            [sx * reach * 0.7, origin.y - drop, origin.z + 34],
          ],
          1.15,
          0.7,
          { segments: Math.round(30 * quality), radialSegments: 8, seed: i * 3 + 7 },
        ),
      );
    });
  }
}

/** Crânio colossal de boca escancarada, na frente do santuário. */
function buildGreatSkull(batch, quality) {
  const skull = buildSkull({ jawOpen: 0.78, detail: quality > 0.7 ? 1 : 0.4 });
  const { skullCenter, skullScale } = SHRINE_METRICS;

  batch.merge(
    skull,
    T({
      pos: skullCenter.toArray(),
      rot: [-0.14, 0, 0],
      scale: skullScale,
    }),
    7,
  );

  // Vértebras cervicais mergulhando na plataforma
  const neck = new THREE.CatmullRomCurve3([
    new THREE.Vector3(0, skullCenter.y - 6, skullCenter.z - 10),
    new THREE.Vector3(0, skullCenter.y - 13, skullCenter.z - 16),
    new THREE.Vector3(0, skullCenter.y - 17, skullCenter.z - 23),
  ]);
  buildSpine(batch, neck, Math.round(7 * quality), () => 5);

  // Presas exageradas nos cantos da boca
  [-1, 1].forEach((sx) => {
    batch.add(
      'tooth',
      spike(9, 1.8, { sharpness: 1.8, bend: sx * 0.12 }),
      T({ pos: [sx * 4.6, skullCenter.y - 9.6, skullCenter.z + 5.4], rot: [0.42, 0, sx * 0.2] }),
    );
    batch.add(
      'tooth',
      spike(7.6, 1.6, { sharpness: 1.8, bend: sx * 0.1 }),
      T({ pos: [sx * 4.4, skullCenter.y - 16.5, skullCenter.z + 4.6], rot: [Math.PI - 0.36, 0, sx * -0.18] }),
    );
  });

  // Chifres saindo da testa
  [-1, 1].forEach((sx) => {
    batch.add(
      'bone',
      spike(18, 2, { sharpness: 1.75, bend: sx * 0.55, lean: -0.3 }),
      T({ pos: [sx * 6.2, skullCenter.y + 8.6, skullCenter.z - 3.4], rot: [-0.52, 0, sx * 0.44] }),
    );
  });
}

/** Os dois braços descarnados que emolduram o domínio. */
function buildArms(batch, quality) {
  [-1, 1].forEach((side) => {
    buildArm(batch, {
      side,
      thickness: 3.3,
      shoulder: [side * 24, 47, -18],
      elbow: [side * 62, 33, 4],
      wrist: [side * 54, 13, 44],
      hand: {
        pos: [side * 47, 5.6, 60],
        yaw: side * -0.28,
        pitch: 0.09,
        roll: side * 0.05,
        scale: 10,
        spread: 1.05,
        curl: 0.24,
      },
    });

    // Ombro reforçado com placas de osso
    for (let i = 0; i < Math.round(5 * quality); i += 1) {
      batch.add(
        'bone',
        spike(13 - i * 1.6, 2.1, { sharpness: 1.6, lean: -0.3 }),
        T({
          pos: [side * (20 + i * 3.4), 49 - i * 2.8, -19 + i * 2.4],
          rot: [-0.5, 0, side * (0.45 + i * 0.16)],
        }),
      );
    }
  });
}

/**
 * Frontões triangulares nas duas pontas do telhado inferior.
 * A cumeeira corre no eixo X, então as empenas ficam em ±X — é ali que o
 * telhado deixa um vão triangular aberto entre o beiral e a cumeeira.
 */
function buildGable(batch, quality) {
  const apex = SHRINE_METRICS.ridgeY - 1.5;
  const base = SHRINE_METRICS.eaveY - 1;
  const halfDepth = 19;

  [-1, 1].forEach((sx) => {
    const x = sx * 33;

    // Montantes verticais preenchendo o triângulo
    const count = Math.round(9 * quality);
    for (let i = 0; i < count; i += 1) {
      const t = i / (count - 1);
      const z = (t * 2 - 1) * halfDepth;
      const height = apex - Math.abs(t * 2 - 1) * (apex - base);
      const bone = span([x, base - 8, z], [x, height, z]);
      if (bone.length < 0.5) continue;

      batch.add(
        'bone',
        longBone(bone.length, 0.8, 0.55, { segments: 14, radialSegments: 8, seed: i + sx * 20 }),
        bone.matrix,
      );
    }

    // Vigas inclinadas do beiral até o topo da empena
    [-1, 1].forEach((sz) => {
      batch.add(
        'bone',
        curvedBone(
          [
            [x, base - 2, sz * halfDepth],
            [x, (base + apex) / 2 + 1, (sz * halfDepth) / 2],
            [x, apex, 0],
          ],
          1.2,
          0.85,
          { segments: Math.round(22 * quality), radialSegments: 9 },
        ),
      );
    });

    // Crânio guardião no vértice da empena
    batch.merge(
      buildSkull({ jawOpen: 0.42, detail: 0.4 }),
      T({ pos: [x, apex - 3.4, 0], rot: [0, sx * Math.PI * 0.5, 0], scale: 3.2 }),
      2.6,
    );
  });
}

/**
 * Constrói o santuário completo.
 * Devolve um Group pronto para entrar na cena.
 */
export function buildMalevolentShrine({ materials, quality = 1 } = {}) {
  const random = makeRandom();
  const batch = new GeoBatch();

  buildPlatform(batch, random, quality);
  buildColonnade(batch, quality);
  buildRibWalls(batch, quality);
  buildGable(batch, quality);
  buildBackbone(batch, quality);

  const lowerRoof = {
    ridgeY: SHRINE_METRICS.ridgeY,
    eaveY: SHRINE_METRICS.eaveY,
    ridgeHalfLen: 42,
    eaveHalfLen: 46,
    halfDepth: 23,
    cornerLift: 8,
    thickness: 2.1,
    hornLength: 17,
  };
  buildRoof(batch, lowerRoof, { rafters: 38, quality });
  buildRidge(batch, { ridgeY: SHRINE_METRICS.ridgeY, ridgeHalfLen: 41, vertebrae: 22, spikeHeight: 6.4, quality });

  buildUpperStory(batch, quality);

  const upperRoof = {
    ridgeY: SHRINE_METRICS.upperRidgeY,
    eaveY: SHRINE_METRICS.upperEaveY,
    ridgeHalfLen: 25,
    eaveHalfLen: 28,
    halfDepth: 14,
    cornerLift: 5.4,
    thickness: 1.6,
    hornLength: 13,
  };
  buildRoof(batch, upperRoof, { rafters: 24, quality });
  buildRidge(batch, { ridgeY: SHRINE_METRICS.upperRidgeY, ridgeHalfLen: 24, vertebrae: 15, spikeHeight: 5.2, quality });

  // Coroa de chifres no topo
  const crownCount = Math.round(9 * quality);
  for (let i = 0; i < crownCount; i += 1) {
    const angle = (i / crownCount) * TAU;
    batch.add(
      'bone',
      spike(11 + Math.cos(angle * 2) * 3, 1.1, { sharpness: 1.8, bend: 0.24 }),
      T({
        pos: [Math.cos(angle) * 5.5, SHRINE_METRICS.upperRidgeY + 2.5, Math.sin(angle) * 5.5],
        rot: [Math.sin(angle) * 0.42, -angle, -Math.cos(angle) * 0.42],
      }),
    );
  }

  buildGreatSkull(batch, quality);
  buildArms(batch, quality);

  const group = batch.build(materials);
  group.name = 'malevolent-shrine';
  group.userData.vertexCount = batch.count();
  return group;
}
