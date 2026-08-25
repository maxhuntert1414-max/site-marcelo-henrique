/**
 * Atmosfera do domínio: céu, chão, brasas, cinzas, cortes e a cúpula de abertura.
 */
import * as THREE from 'three';
import { createSparkTexture, createSlashTexture } from './textures.js';

const GLSL_NOISE = /* glsl */ `
  float hash21(vec2 p) {
    p = fract(p * vec2(123.34, 456.21));
    p += dot(p, p + 45.32);
    return fract(p.x * p.y);
  }

  float valueNoise(vec2 p) {
    vec2 i = floor(p);
    vec2 f = fract(p);
    vec2 u = f * f * (3.0 - 2.0 * f);
    float a = hash21(i);
    float b = hash21(i + vec2(1.0, 0.0));
    float c = hash21(i + vec2(0.0, 1.0));
    float d = hash21(i + vec2(1.0, 1.0));
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
  }

  float fbm(vec2 p) {
    float sum = 0.0;
    float amp = 0.5;
    for (int i = 0; i < 5; i++) {
      sum += amp * valueNoise(p);
      p *= 2.02;
      amp *= 0.5;
    }
    return sum;
  }
`;

/** Céu do domínio: vermelho profundo no horizonte, breu no zênite, nuvens lentas. */
export function createSky(radius = 900) {
  const material = new THREE.ShaderMaterial({
    side: THREE.BackSide,
    depthWrite: false,
    fog: false,
    uniforms: {
      uTime: { value: 0 },
      uHorizon: { value: new THREE.Color(0x330705) },
      uZenith: { value: new THREE.Color(0x030204) },
      uGlow: { value: new THREE.Color(0xff3a12) },
      uIntensity: { value: 1 },
    },
    vertexShader: /* glsl */ `
      varying vec3 vDir;
      void main() {
        vDir = normalize(position);
        gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
      }
    `,
    fragmentShader: /* glsl */ `
      varying vec3 vDir;
      uniform float uTime;
      uniform float uIntensity;
      uniform vec3 uHorizon;
      uniform vec3 uZenith;
      uniform vec3 uGlow;
      ${GLSL_NOISE}

      void main() {
        vec3 dir = normalize(vDir);
        float h = dir.y;

        float band = smoothstep(-0.35, 0.75, h);
        vec3 color = mix(uHorizon, uZenith, pow(band, 0.75));

        // Nuvens de energia amaldiçoada girando devagar
        vec2 p = vec2(atan(dir.z, dir.x) * 1.6, h * 2.6);
        float clouds = fbm(p * 1.7 + vec2(uTime * 0.014, uTime * 0.006));
        clouds = pow(clouds, 2.6);
        color += uGlow * clouds * 0.22 * (1.0 - band) * uIntensity;

        // Brasa no horizonte
        float horizon = exp(-abs(h) * 7.0);
        color += uHorizon * horizon * 0.55 * uIntensity;

        // Abismo abaixo da linha do chão
        color *= smoothstep(-0.75, -0.05, h) * 0.85 + 0.15;

        gl_FragColor = vec4(color * uIntensity, 1.0);
      }
    `,
  });

  const mesh = new THREE.Mesh(new THREE.SphereGeometry(radius, 48, 32), material);
  mesh.name = 'sky';
  mesh.frustumCulled = false;
  return mesh;
}

/** Chão de cinza com fendas incandescentes e queda para o breu. */
export function createGround(textures) {
  const material = new THREE.MeshStandardMaterial({
    map: textures.map,
    normalMap: textures.normalMap,
    emissiveMap: textures.emissiveMap,
    emissive: new THREE.Color(0xff5a24),
    emissiveIntensity: 0.042,
    roughness: 0.97,
    metalness: 0.02,
    color: new THREE.Color(0x7c7874),
    normalScale: new THREE.Vector2(0.9, 0.9),
  });

  [textures.map, textures.normalMap, textures.emissiveMap].forEach((texture) => {
    texture.wrapS = THREE.RepeatWrapping;
    texture.wrapT = THREE.RepeatWrapping;
    texture.repeat.set(9, 9);
  });

  const geometry = new THREE.CircleGeometry(460, 96);
  geometry.rotateX(-Math.PI / 2);

  const mesh = new THREE.Mesh(geometry, material);
  mesh.receiveShadow = true;
  mesh.name = 'ground';
  return mesh;
}

function makeParticleMaterial(texture, { size, color, opacity, blending }) {
  return new THREE.ShaderMaterial({
    transparent: true,
    depthWrite: false,
    blending,
    uniforms: {
      uTime: { value: 0 },
      uTexture: { value: texture },
      uSize: { value: size },
      uColor: { value: new THREE.Color(color) },
      uOpacity: { value: opacity },
      uPixelRatio: { value: 1 },
    },
    vertexShader: /* glsl */ `
      attribute float aSeed;
      attribute float aSpeed;
      attribute float aScale;
      attribute float aSway;
      uniform float uTime;
      uniform float uSize;
      uniform float uPixelRatio;
      varying float vLife;

      void main() {
        float life = fract(aSeed + uTime * aSpeed);
        vec3 pos = position;
        pos.y += life * aSway * 6.0 + life * 120.0 * aSpeed * 4.0;
        pos.x += sin(uTime * 0.5 + aSeed * 42.0) * aSway;
        pos.z += cos(uTime * 0.42 + aSeed * 27.0) * aSway;

        vLife = sin(life * 3.14159265);

        vec4 mv = modelViewMatrix * vec4(pos, 1.0);
        gl_Position = projectionMatrix * mv;
        gl_PointSize = uSize * aScale * uPixelRatio * (260.0 / max(-mv.z, 1.0));
      }
    `,
    fragmentShader: /* glsl */ `
      uniform sampler2D uTexture;
      uniform vec3 uColor;
      uniform float uOpacity;
      varying float vLife;

      void main() {
        vec4 tex = texture2D(uTexture, gl_PointCoord);
        float alpha = tex.a * vLife * uOpacity;
        if (alpha < 0.01) discard;
        gl_FragColor = vec4(uColor * tex.rgb, alpha);
      }
    `,
  });
}

function particleField(count, spawn, materialOptions, texture) {
  const positions = new Float32Array(count * 3);
  const seeds = new Float32Array(count);
  const speeds = new Float32Array(count);
  const scales = new Float32Array(count);
  const sways = new Float32Array(count);

  for (let i = 0; i < count; i += 1) {
    const p = spawn(i, count);
    positions[i * 3] = p.x;
    positions[i * 3 + 1] = p.y;
    positions[i * 3 + 2] = p.z;
    seeds[i] = Math.random();
    speeds[i] = p.speed;
    scales[i] = p.scale;
    sways[i] = p.sway;
  }

  const geometry = new THREE.BufferGeometry();
  geometry.setAttribute('position', new THREE.BufferAttribute(positions, 3));
  geometry.setAttribute('aSeed', new THREE.BufferAttribute(seeds, 1));
  geometry.setAttribute('aSpeed', new THREE.BufferAttribute(speeds, 1));
  geometry.setAttribute('aScale', new THREE.BufferAttribute(scales, 1));
  geometry.setAttribute('aSway', new THREE.BufferAttribute(sways, 1));
  geometry.boundingSphere = new THREE.Sphere(new THREE.Vector3(0, 60, 0), 600);

  const points = new THREE.Points(geometry, makeParticleMaterial(texture, materialOptions));
  points.frustumCulled = false;
  return points;
}

/** Brasas subindo do santuário. */
export function createEmbers(count = 900) {
  const texture = createSparkTexture();

  return particleField(
    count,
    () => {
      const angle = Math.random() * Math.PI * 2;
      const radius = 8 + Math.pow(Math.random(), 0.6) * 110;
      return {
        x: Math.cos(angle) * radius,
        y: -6 + Math.random() * 24,
        z: Math.sin(angle) * radius * 0.9,
        speed: 0.006 + Math.random() * 0.016,
        scale: 0.4 + Math.random() * 1.5,
        sway: 2 + Math.random() * 9,
      };
    },
    { size: 5.5, color: 0xff5c1c, opacity: 0.6, blending: THREE.AdditiveBlending },
    texture,
  );
}

/** Cinza pairando no ar, mais lenta e fria. */
export function createAsh(count = 700) {
  const texture = createSparkTexture();

  return particleField(
    count,
    () => {
      const angle = Math.random() * Math.PI * 2;
      const radius = Math.pow(Math.random(), 0.5) * 260;
      return {
        x: Math.cos(angle) * radius,
        y: Math.random() * 150,
        z: Math.sin(angle) * radius,
        speed: 0.002 + Math.random() * 0.004,
        scale: 0.25 + Math.random() * 0.8,
        sway: 6 + Math.random() * 16,
      };
    },
    { size: 3.4, color: 0x8d7f74, opacity: 0.22, blending: THREE.NormalBlending },
    texture,
  );
}

/**
 * "Desmantelar": os cortes automáticos que rasgam o espaço dentro do domínio.
 * Pool de lâminas reaproveitadas, cada uma com seu próprio material aditivo.
 */
export class CleaveField {
  constructor({ count = 26, radius = 90, height = 90 } = {}) {
    this.group = new THREE.Group();
    this.group.name = 'cleave-field';
    this.radius = radius;
    this.height = height;
    this.blades = [];
    this.timer = 0;
    this.rate = 0.11;
    this.enabled = true;
    this.intensity = 1;

    const texture = createSlashTexture();
    const geometry = new THREE.PlaneGeometry(1, 1);

    for (let i = 0; i < count; i += 1) {
      const material = new THREE.MeshBasicMaterial({
        map: texture,
        transparent: true,
        depthWrite: false,
        blending: THREE.AdditiveBlending,
        side: THREE.DoubleSide,
        opacity: 0,
        color: new THREE.Color(0xfff0e2),
      });

      const mesh = new THREE.Mesh(geometry, material);
      mesh.visible = false;
      mesh.frustumCulled = false;
      this.group.add(mesh);
      this.blades.push({ mesh, material, life: 0, duration: 1, length: 1, width: 1 });
    }
  }

  spawn(blade) {
    const angle = Math.random() * Math.PI * 2;
    const distance = Math.pow(Math.random(), 0.55) * this.radius;

    blade.mesh.position.set(
      Math.cos(angle) * distance,
      6 + Math.random() * this.height,
      Math.sin(angle) * distance + 6,
    );
    blade.mesh.rotation.set(
      (Math.random() - 0.5) * 1.5,
      Math.random() * Math.PI * 2,
      (Math.random() - 0.5) * Math.PI,
    );

    blade.length = 22 + Math.random() * 54;
    blade.width = 0.7 + Math.random() * 2.6;
    blade.duration = 0.13 + Math.random() * 0.18;
    blade.life = 0;
    blade.mesh.visible = true;
    blade.material.color.setHSL(0.05 + Math.random() * 0.03, 0.08 + Math.random() * 0.22, 0.9);
  }

  update(delta, elapsed) {
    if (this.enabled) {
      this.timer -= delta;
      if (this.timer <= 0) {
        this.timer = this.rate * (0.35 + Math.random()) / Math.max(this.intensity, 0.05);
        const bursts = 1 + Math.floor(Math.random() * 3 * this.intensity);
        for (let i = 0; i < bursts; i += 1) {
          const blade = this.blades.find((b) => !b.mesh.visible);
          if (blade) this.spawn(blade);
        }
      }
    }

    this.blades.forEach((blade) => {
      if (!blade.mesh.visible) return;

      blade.life += delta;
      const t = blade.life / blade.duration;

      if (t >= 1) {
        blade.mesh.visible = false;
        blade.material.opacity = 0;
        return;
      }

      const grow = Math.min(t / 0.18, 1);
      const fade = t < 0.22 ? t / 0.22 : Math.pow(1 - (t - 0.22) / 0.78, 1.7);

      blade.mesh.scale.set(blade.length * (0.35 + grow * 0.65), blade.width, 1);
      blade.material.opacity = fade * 1.6;
    });

    // Leve deriva do conjunto, como se o espaço estivesse instável
    this.group.rotation.y = Math.sin(elapsed * 0.07) * 0.06;
  }

  setEnabled(value) {
    this.enabled = value;
    if (!value) {
      this.blades.forEach((blade) => {
        blade.mesh.visible = false;
        blade.material.opacity = 0;
      });
    }
  }
}

/**
 * Cúpula de abertura do domínio: a esfera negra que se expande e some.
 */
export function createDomainDome() {
  const material = new THREE.ShaderMaterial({
    side: THREE.BackSide,
    transparent: true,
    depthWrite: false,
    fog: false,
    uniforms: {
      uTime: { value: 0 },
      uProgress: { value: 0 },
      uOpacity: { value: 0 },
    },
    vertexShader: /* glsl */ `
      varying vec3 vNormalW;
      varying vec3 vPos;
      void main() {
        vNormalW = normalize(mat3(modelMatrix) * normal);
        vPos = position;
        gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
      }
    `,
    fragmentShader: /* glsl */ `
      varying vec3 vNormalW;
      varying vec3 vPos;
      uniform float uTime;
      uniform float uProgress;
      uniform float uOpacity;
      ${GLSL_NOISE}

      void main() {
        vec3 dir = normalize(vPos);
        vec2 p = vec2(atan(dir.z, dir.x) * 2.2, dir.y * 3.0);
        float veins = fbm(p * 2.4 + vec2(uTime * 0.35, uTime * 0.12));
        veins = pow(veins, 3.2);

        float edge = pow(1.0 - abs(dir.y), 2.0);
        vec3 color = vec3(0.02, 0.005, 0.01);
        color += vec3(1.0, 0.16, 0.05) * veins * 2.2;
        color += vec3(0.9, 0.1, 0.04) * edge * uProgress * 0.6;

        gl_FragColor = vec4(color, uOpacity);
      }
    `,
  });

  const mesh = new THREE.Mesh(new THREE.SphereGeometry(1, 48, 32), material);
  mesh.name = 'domain-dome';
  mesh.visible = false;
  mesh.frustumCulled = false;
  return mesh;
}

/** Onda de choque no chão, disparada na abertura do domínio. */
export function createShockwave() {
  const material = new THREE.ShaderMaterial({
    transparent: true,
    depthWrite: false,
    blending: THREE.AdditiveBlending,
    side: THREE.DoubleSide,
    fog: false,
    uniforms: { uProgress: { value: 0 }, uOpacity: { value: 0 } },
    vertexShader: /* glsl */ `
      varying vec2 vUv;
      void main() {
        vUv = uv;
        gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
      }
    `,
    fragmentShader: /* glsl */ `
      varying vec2 vUv;
      uniform float uProgress;
      uniform float uOpacity;
      void main() {
        float d = distance(vUv, vec2(0.5)) * 2.0;
        float ring = smoothstep(uProgress - 0.06, uProgress, d) * (1.0 - smoothstep(uProgress, uProgress + 0.03, d));
        float glow = ring * (1.0 - uProgress);
        gl_FragColor = vec4(vec3(1.0, 0.32, 0.1) * glow * 3.0, glow * uOpacity);
      }
    `,
  });

  const geometry = new THREE.PlaneGeometry(900, 900);
  geometry.rotateX(-Math.PI / 2);
  const mesh = new THREE.Mesh(geometry, material);
  mesh.position.y = 0.6;
  mesh.visible = false;
  mesh.frustumCulled = false;
  return mesh;
}
