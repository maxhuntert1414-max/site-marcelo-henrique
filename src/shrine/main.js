/**
 * 伏魔御廚子 — Malevolent Shrine
 * Simulação 3D em tempo real, 100% procedural (nenhum modelo ou textura externa).
 */
import * as THREE from 'three';
import { OrbitControls } from 'three/examples/jsm/controls/OrbitControls.js';
import { EffectComposer } from 'three/examples/jsm/postprocessing/EffectComposer.js';
import { RenderPass } from 'three/examples/jsm/postprocessing/RenderPass.js';
import { UnrealBloomPass } from 'three/examples/jsm/postprocessing/UnrealBloomPass.js';
import { OutputPass } from 'three/examples/jsm/postprocessing/OutputPass.js';

import './shrine.css';
import { createShrineMaterials } from './materials.js';
import { buildMalevolentShrine, SHRINE_METRICS } from './shrine.js';
import { T } from './bones.js';
import { createGroundTextures, createSparkTexture } from './textures.js';
import {
  createSky,
  createGround,
  createEmbers,
  createAsh,
  createDomainDome,
  createShockwave,
  CleaveField,
} from './effects.js';

const $ = (selector) => document.querySelector(selector);
const clamp = (v, a, b) => Math.min(Math.max(v, a), b);
const easeInOut = (t) => (t < 0.5 ? 4 * t * t * t : 1 - Math.pow(-2 * t + 2, 3) / 2);
const nextFrame = () => new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve)));

const prefersReducedMotion = window.matchMedia('(prefers-reduced-motion: reduce)').matches;
const isEmbedded = window.self !== window.top;

const CAMERAS = {
  dominio: { position: [0, 36, 182], target: [0, 32, 6] },
  ritual: { position: [0, 10, 142], target: [0, 40, 14] },
  lateral: { position: [148, 44, 74], target: [0, 30, 0] },
  aerea: { position: [8, 148, 138], target: [0, 24, 0] },
  cranio: { position: [0, 28, 116], target: [0, 20, 50] },
};

function supportsWebGL() {
  try {
    const canvas = document.createElement('canvas');
    return Boolean(
      window.WebGLRenderingContext && (canvas.getContext('webgl2') || canvas.getContext('webgl')),
    );
  } catch {
    return false;
  }
}

function detectQuality() {
  const isMobile = window.matchMedia('(max-width: 820px)').matches || navigator.maxTouchPoints > 1;
  const cores = navigator.hardwareConcurrency ?? 4;
  const small = window.innerWidth * window.innerHeight < 700_000;

  if (isMobile || cores <= 4 || small) {
    return { level: 'leve', geometry: 0.62, embers: 380, ash: 260, shadows: false, bloom: 0.36, pixelRatio: 1.25, textureSize: 256 };
  }
  if (cores <= 8) {
    return { level: 'equilibrada', geometry: 0.85, embers: 700, ash: 480, shadows: true, bloom: 0.42, pixelRatio: 1.5, textureSize: 512 };
  }
  return { level: 'alta', geometry: 1, embers: 1000, ash: 700, shadows: true, bloom: 0.46, pixelRatio: 1.75, textureSize: 512 };
}

async function boot() {
  const canvas = $('#shrine-canvas');
  const loader = $('#loader');
  const loaderStatus = $('#loader-status');
  const loaderBar = $('#loader-bar');
  const stage = $('#stage');

  if (!supportsWebGL()) {
    stage.classList.add('no-webgl');
    loader.classList.add('is-hidden');
    return;
  }

  const quality = detectQuality();
  let progress = 0;

  const step = async (label, fn) => {
    loaderStatus.textContent = label;
    progress += 1;
    loaderBar.style.width = `${Math.min((progress / 7) * 100, 100)}%`;
    await nextFrame();
    return fn();
  };

  // --- Renderizador ------------------------------------------------------
  const renderer = new THREE.WebGLRenderer({
    canvas,
    antialias: quality.level !== 'leve',
    powerPreference: 'high-performance',
    stencil: false,
  });
  renderer.setPixelRatio(Math.min(window.devicePixelRatio, quality.pixelRatio));
  renderer.setSize(window.innerWidth, window.innerHeight);
  renderer.toneMapping = THREE.ACESFilmicToneMapping;
  renderer.toneMappingExposure = 1.0;
  renderer.outputColorSpace = THREE.SRGBColorSpace;
  renderer.shadowMap.enabled = quality.shadows;
  renderer.shadowMap.type = THREE.PCFSoftShadowMap;

  const scene = new THREE.Scene();
  scene.fog = new THREE.FogExp2(0x070307, 0.0016);

  const camera = new THREE.PerspectiveCamera(46, window.innerWidth / window.innerHeight, 0.5, 2200);
  camera.position.set(0, 66, 430);

  const controls = new OrbitControls(camera, canvas);
  controls.enableDamping = true;
  controls.dampingFactor = 0.055;
  controls.target.set(0, 30, 4);
  controls.minDistance = 34;
  controls.maxDistance = 460;
  controls.maxPolarAngle = Math.PI * 0.495;
  controls.autoRotateSpeed = 0.4;
  controls.enabled = false;

  // --- Luzes -------------------------------------------------------------
  const hemi = new THREE.HemisphereLight(0x2a1210, 0x090708, 0.3);
  scene.add(hemi);

  const key = new THREE.DirectionalLight(0xfff4e8, 5.6);
  key.position.set(86, 150, 150);
  key.castShadow = quality.shadows;
  if (quality.shadows) {
    key.shadow.mapSize.set(2048, 2048);
    key.shadow.camera.near = 20;
    key.shadow.camera.far = 520;
    key.shadow.camera.left = -140;
    key.shadow.camera.right = 140;
    key.shadow.camera.top = 150;
    key.shadow.camera.bottom = -60;
    key.shadow.bias = -0.0006;
    key.shadow.normalBias = 0.8;
  }
  scene.add(key);

  const fill = new THREE.DirectionalLight(0x9aabc2, 0.7);
  fill.position.set(-150, 74, -60);
  scene.add(fill);

  const rimA = new THREE.PointLight(0xff4614, 5200, 300, 2);
  rimA.position.set(0, 62, -96);
  const rimB = new THREE.PointLight(0xff6a26, 1800, 220, 2);
  rimB.position.set(-108, 34, -40);
  const rimC = new THREE.PointLight(0xff6a26, 1800, 220, 2);
  rimC.position.set(108, 34, -40);
  scene.add(rimA, rimB, rimC);

  // --- Céu e chão --------------------------------------------------------
  const sky = await step('Abrindo o véu do domínio…', () => {
    const mesh = createSky(1000);
    scene.add(mesh);
    return mesh;
  });

  const ground = await step('Queimando o solo…', () => {
    const mesh = createGround(createGroundTextures(quality.textureSize));
    scene.add(mesh);
    return mesh;
  });

  // --- Santuário ---------------------------------------------------------
  const materials = await step('Curtindo o osso…', () => createShrineMaterials({ textureSize: quality.textureSize }));

  const shrine = await step('Erguendo o santuário…', () =>
    buildMalevolentShrine({ materials, quality: quality.geometry }),
  );
  scene.add(shrine);

  // --- Brilho nas órbitas e na goela do crânio ---------------------------
  const skullMatrix = T({
    pos: SHRINE_METRICS.skullCenter.toArray(),
    rot: [-0.16, 0, 0],
    scale: SHRINE_METRICS.skullScale,
  });
  const localToWorld = (x, y, z) => new THREE.Vector3(x, y, z).applyMatrix4(skullMatrix);

  const sparkTexture = createSparkTexture(128);
  const eyeGlows = [];
  const eyeLights = [];

  [-1, 1].forEach((side) => {
    const at = localToWorld(side * 0.27, 0.03, 0.44);

    const sprite = new THREE.Sprite(
      new THREE.SpriteMaterial({
        map: sparkTexture,
        color: 0xff3c0c,
        transparent: true,
        blending: THREE.AdditiveBlending,
        depthWrite: false,
      }),
    );
    sprite.position.copy(at);
    sprite.scale.setScalar(4.5);
    scene.add(sprite);
    eyeGlows.push(sprite);

    const light = new THREE.PointLight(0xff3c0c, 700, 80, 2);
    light.position.copy(at);
    scene.add(light);
    eyeLights.push(light);
  });

  const mouth = localToWorld(0, -0.82, 0.3);
  const mouthLight = new THREE.PointLight(0xff5a18, 2400, 130, 2);
  mouthLight.position.copy(mouth);
  scene.add(mouthLight);

  const mouthGlow = new THREE.Sprite(
    new THREE.SpriteMaterial({
      map: sparkTexture,
      color: 0xff4a10,
      transparent: true,
      blending: THREE.AdditiveBlending,
      depthWrite: false,
      opacity: 0.42,
    }),
  );
  mouthGlow.position.copy(mouth);
  mouthGlow.scale.setScalar(20);
  scene.add(mouthGlow);

  // --- Partículas e cortes ----------------------------------------------
  const { embers, ash, cleave, dome, shockwave } = await step('Acendendo as brasas…', () => {
    const embersField = createEmbers(quality.embers);
    const ashField = createAsh(quality.ash);
    const cleaveField = new CleaveField({ count: quality.level === 'leve' ? 16 : 28, radius: 96, height: 96 });
    const domeMesh = createDomainDome();
    const wave = createShockwave();

    scene.add(embersField, ashField, cleaveField.group, domeMesh, wave);
    return { embers: embersField, ash: ashField, cleave: cleaveField, dome: domeMesh, shockwave: wave };
  });

  embers.material.uniforms.uPixelRatio.value = renderer.getPixelRatio();
  ash.material.uniforms.uPixelRatio.value = renderer.getPixelRatio();

  // --- Pós-processamento -------------------------------------------------
  const composer = await step('Selando o domínio…', () => {
    const c = new EffectComposer(renderer);
    c.addPass(new RenderPass(scene, camera));

    const bloom = new UnrealBloomPass(
      new THREE.Vector2(window.innerWidth, window.innerHeight),
      quality.bloom,
      0.66,
      0.86,
    );
    c.addPass(bloom);
    c.addPass(new OutputPass());
    c.userData = { bloom };
    return c;
  });

  const bloomPass = composer.userData.bloom;

  // --- Estado da interface ----------------------------------------------
  const state = {
    autoRotate: !prefersReducedMotion,
    cleave: true,
    embers: true,
    bloom: true,
    intro: !prefersReducedMotion,
    introTime: 0,
    fly: null,
  };

  controls.autoRotate = state.autoRotate;

  /**
   * Posição de um preset já corrigida pela proporção da janela.
   * Os presets foram enquadrados em paisagem; em retrato o campo de visão
   * horizontal encolhe, então afastamos a câmera na mesma medida.
   */
  const presetPosition = (preset) => {
    const target = new THREE.Vector3().fromArray(preset.target);
    const position = new THREE.Vector3().fromArray(preset.position);
    // Em retrato não dá para caber a largura inteira sem deixar o santuário
    // minúsculo: recuamos até um limite e deixamos as mãos saírem do quadro.
    const pullBack = clamp(1.2 / Math.max(camera.aspect, 0.2), 1, 1.8);
    return { position: target.clone().addScaledVector(position.sub(target), pullBack), target };
  };

  const flyTo = (name, duration = 1.6) => {
    const preset = CAMERAS[name];
    if (!preset) return;

    const framed = presetPosition(preset);
    state.fly = {
      time: 0,
      duration: prefersReducedMotion ? 0.01 : duration,
      fromPos: camera.position.clone(),
      toPos: framed.position,
      fromTarget: controls.target.clone(),
      toTarget: framed.target,
    };
  };

  const setIntroStage = (stageName) => {
    const intro = $('#intro');
    if (intro) intro.dataset.stage = stageName;
  };

  const startIntro = () => {
    state.intro = true;
    state.introTime = 0;
    controls.enabled = false;
    controls.autoRotate = false;
    dome.visible = true;
    dome.material.uniforms.uOpacity.value = 0;
    shockwave.visible = true;
    camera.position.set(0, 66, 430);
    controls.target.set(0, 30, 4);
    document.body.classList.add('is-intro');
    setIntroStage('0');
  };

  const finishIntro = () => {
    state.intro = false;
    state.fly = null;
    dome.visible = false;
    shockwave.visible = false;
    controls.enabled = true;
    controls.autoRotate = state.autoRotate;
    const framed = presetPosition(CAMERAS.dominio);
    camera.position.copy(framed.position);
    controls.target.copy(framed.target);
    document.body.classList.remove('is-intro');
    setIntroStage('done');
  };

  const introCues = [
    { at: 0.15, run: () => setIntroStage('1') },
    { at: 2.0, run: () => setIntroStage('2') },
    {
      at: 2.35,
      run: () => {
        dome.visible = true;
        shockwave.visible = true;
        const framed = presetPosition(CAMERAS.dominio);
        state.fly = {
          time: 0,
          duration: 5.6,
          fromPos: camera.position.clone(),
          toPos: framed.position,
          fromTarget: controls.target.clone(),
          toTarget: framed.target,
        };
      },
    },
    { at: 5.4, run: () => setIntroStage('3') },
    { at: 8.4, run: () => finishIntro() },
  ];
  let cueIndex = 0;

  const updateIntro = (delta) => {
    state.introTime += delta;

    while (cueIndex < introCues.length && state.introTime >= introCues[cueIndex].at) {
      introCues[cueIndex].run();
      cueIndex += 1;
    }

    const t = state.introTime;

    if (dome.visible) {
      const p = clamp((t - 2.35) / 2.6, 0, 1);
      const radius = 6 + easeInOut(p) * 320;
      dome.scale.setScalar(radius);
      dome.position.set(0, 30, 0);
      dome.material.uniforms.uProgress.value = p;
      dome.material.uniforms.uOpacity.value = p < 0.12 ? p / 0.12 : Math.pow(1 - (p - 0.12) / 0.88, 1.5);
      if (p >= 1) dome.visible = false;
    }

    if (shockwave.visible) {
      const p = clamp((t - 2.4) / 2.2, 0, 1);
      shockwave.material.uniforms.uProgress.value = p;
      shockwave.material.uniforms.uOpacity.value = 1 - p;
      if (p >= 1) shockwave.visible = false;
    }
  };

  // --- Controles da interface -------------------------------------------
  const bindToggle = (id, initial, onChange) => {
    const button = $(id);
    if (!button) return;
    const apply = (value) => {
      button.setAttribute('aria-pressed', String(value));
      button.dataset.state = value ? 'on' : 'off';
      onChange(value);
    };
    apply(initial);
    button.addEventListener('click', () => {
      const next = button.getAttribute('aria-pressed') !== 'true';
      apply(next);
    });
  };

  bindToggle('#toggle-rotate', state.autoRotate, (value) => {
    state.autoRotate = value;
    if (!state.intro) controls.autoRotate = value;
  });

  bindToggle('#toggle-cleave', true, (value) => {
    state.cleave = value;
    cleave.setEnabled(value);
  });

  bindToggle('#toggle-embers', true, (value) => {
    state.embers = value;
    embers.visible = value;
    ash.visible = value;
  });

  bindToggle('#toggle-bloom', true, (value) => {
    state.bloom = value;
    bloomPass.enabled = value;
    bloomPass.strength = value ? quality.bloom : 0;
  });

  document.querySelectorAll('[data-camera]').forEach((button) => {
    button.addEventListener('click', () => {
      document.querySelectorAll('[data-camera]').forEach((b) => b.classList.remove('is-active'));
      button.classList.add('is-active');
      flyTo(button.dataset.camera);
    });
  });

  $('#replay')?.addEventListener('click', () => {
    cueIndex = 0;
    startIntro();
  });

  $('#skip-intro')?.addEventListener('click', () => {
    cueIndex = introCues.length;
    finishIntro();
  });

  const captureButton = $('#capture');
  if (isEmbedded) {
    captureButton?.remove();
  } else {
    captureButton?.addEventListener('click', () => {
      composer.render();
      const url = renderer.domElement.toDataURL('image/png');
      const link = document.createElement('a');
      link.href = url;
      link.download = 'malevolent-shrine.png';
      link.click();
    });
  }

  $('#fullscreen')?.addEventListener('click', () => {
    if (document.fullscreenElement) document.exitFullscreen();
    else document.documentElement.requestFullscreen?.();
  });

  const panel = $('#panel');
  $('#toggle-panel')?.addEventListener('click', () => {
    panel?.classList.toggle('is-collapsed');
  });

  if (window.matchMedia('(max-width: 820px)').matches) {
    panel?.classList.add('is-collapsed');
  }

  window.addEventListener('keydown', (event) => {
    if (event.target instanceof HTMLInputElement) return;
    const map = { 1: 'dominio', 2: 'ritual', 3: 'lateral', 4: 'aerea', 5: 'cranio' };

    if (map[event.key]) {
      flyTo(map[event.key]);
      return;
    }

    if (event.code === 'Space') {
      event.preventDefault();
      $('#toggle-rotate')?.click();
    }

    if (event.key.toLowerCase() === 'c') $('#toggle-cleave')?.click();
    if (event.key === 'Escape' && state.intro) $('#skip-intro')?.click();
  });

  // --- Redimensionamento -------------------------------------------------
  const resize = () => {
    const width = window.innerWidth;
    const height = window.innerHeight;
    camera.aspect = width / height;
    camera.updateProjectionMatrix();
    renderer.setSize(width, height);
    composer.setSize(width, height);
    bloomPass.setSize(width, height);
  };
  window.addEventListener('resize', resize);

  // --- Laço principal ----------------------------------------------------
  const clock = new THREE.Clock();
  const fpsLabel = $('#fps');
  const infoLabel = $('#info-verts');
  if (infoLabel) {
    infoLabel.textContent = `${Math.round(shrine.userData.vertexCount / 1000)}k vértices · qualidade ${quality.level}`;
  }

  let frames = 0;
  let fpsSince = performance.now();
  let raf = 0;

  const animate = () => {
    raf = requestAnimationFrame(animate);

    const realDelta = clock.getDelta();
    // Limita o passo da simulação para nada explodir depois de uma pausa,
    // mas guarda o tempo real para as transições de câmera e da abertura.
    const delta = Math.min(realDelta, 0.06);
    const elapsed = clock.getElapsedTime();

    if (state.intro) updateIntro(realDelta);

    if (state.fly) {
      state.fly.time += realDelta;
      const t = clamp(state.fly.time / state.fly.duration, 0, 1);
      const e = easeInOut(t);
      camera.position.lerpVectors(state.fly.fromPos, state.fly.toPos, e);
      controls.target.lerpVectors(state.fly.fromTarget, state.fly.toTarget, e);
      if (t >= 1) state.fly = null;
    }

    sky.material.uniforms.uTime.value = elapsed;
    dome.material.uniforms.uTime.value = elapsed;
    embers.material.uniforms.uTime.value = elapsed;
    ash.material.uniforms.uTime.value = elapsed;

    cleave.update(delta, elapsed);

    // Respiração do domínio
    const pulse = 0.72 + Math.sin(elapsed * 1.7) * 0.14 + Math.sin(elapsed * 4.3) * 0.06;
    eyeGlows.forEach((sprite, i) => {
      sprite.scale.setScalar(4.2 + pulse * 1.8 + Math.sin(elapsed * 3.1 + i) * 0.5);
      sprite.material.opacity = 0.22 + pulse * 0.16;
    });
    eyeLights.forEach((light) => {
      light.intensity = 300 + pulse * 380;
    });
    mouthLight.intensity = 1900 + pulse * 1500;
    mouthGlow.material.opacity = 0.18 + pulse * 0.14;
    mouthGlow.scale.setScalar(15 + pulse * 5);
    rimA.intensity = 4400 + pulse * 2000;
    materials.void.emissiveIntensity = 0.04 + pulse * 0.06;

    controls.update();
    composer.render();

    frames += 1;
    const now = performance.now();
    if (now - fpsSince >= 600 && fpsLabel) {
      fpsLabel.textContent = `${Math.round((frames * 1000) / (now - fpsSince))} fps`;
      frames = 0;
      fpsSince = now;
    }
  };

  await step('Pronto.', () => {});

  loader.classList.add('is-hidden');
  stage.classList.add('is-ready');

  if (state.intro) startIntro();
  else finishIntro();

  animate();

  document.addEventListener('visibilitychange', () => {
    if (document.hidden) {
      cancelAnimationFrame(raf);
    } else {
      clock.getDelta();
      animate();
    }
  });
}

boot().catch((error) => {
  console.error(error);
  const loaderStatus = $('#loader-status');
  if (loaderStatus) loaderStatus.textContent = `Falha ao abrir o domínio: ${error.message}`;
});
