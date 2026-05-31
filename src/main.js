import * as THREE from 'three';
import './styles.css';

const prefersReducedMotion = window.matchMedia('(prefers-reduced-motion: reduce)').matches;

function supportsWebGL() {
  try {
    const canvas = document.createElement('canvas');
    return Boolean(
      window.WebGLRenderingContext &&
        (canvas.getContext('webgl') || canvas.getContext('experimental-webgl')),
    );
  } catch {
    return false;
  }
}

function initReveal() {
  const items = document.querySelectorAll('.reveal');
  const observer = new IntersectionObserver(
    (entries) => {
      entries.forEach((entry) => {
        if (entry.isIntersecting) {
          entry.target.classList.add('is-visible');
          observer.unobserve(entry.target);
        }
      });
    },
    { threshold: 0.16 },
  );

  items.forEach((item) => observer.observe(item));
}

function initBriefForm() {
  const form = document.querySelector('.brief-form');
  const output = document.querySelector('#message-output');
  const copyButton = document.querySelector('#copy-message');

  if (!form || !output || !copyButton) return;

  const buildMessage = () => {
    const formData = new FormData(form);
    const name = String(formData.get('name') || '').trim();
    const pain = String(formData.get('pain') || '').trim();
    const channel = String(formData.get('channel') || '').trim();

    const who = name ? `Aqui é ${name}.` : 'Aqui é uma pessoa interessada.';
    const problem = pain
      ? `O que está pesando hoje: ${pain}.`
      : 'Quero entender como um site ou automação pode aliviar meu trabalho.';
    const contact = channel ? `Meu melhor canal é: ${channel}.` : 'Podemos combinar o melhor canal de conversa.';

    return `Olá, Marcelo Henrique Santos Sampaio! ${who} ${problem} ${contact} Quero saber valores e próximos passos.`;
  };

  form.addEventListener('submit', (event) => {
    event.preventDefault();
    output.value = buildMessage();
    output.classList.add('is-ready');
  });

  copyButton.addEventListener('click', async () => {
    output.value = output.value || buildMessage();
    const text = output.value;

    try {
      await navigator.clipboard.writeText(text);
      copyButton.textContent = 'Copiado';
      setTimeout(() => {
        copyButton.textContent = 'Copiar';
      }, 1600);
    } catch {
      output.focus();
      document.getSelection()?.selectAllChildren(output);
    }
  });
}

function initHexScene() {
  const canvas = document.querySelector('#hex-canvas');
  const hero = document.querySelector('.hero');

  if (!canvas || !hero) return;

  if (!supportsWebGL()) {
    hero.classList.add('no-webgl');
    return;
  }

  const scene = new THREE.Scene();
  const camera = new THREE.PerspectiveCamera(42, 1, 0.1, 100);
  camera.position.set(0, 0, 14);

  const renderer = new THREE.WebGLRenderer({
    canvas,
    alpha: true,
    antialias: true,
    powerPreference: 'high-performance',
  });

  const isMobile = window.matchMedia('(max-width: 720px)').matches;
  renderer.setPixelRatio(Math.min(window.devicePixelRatio, isMobile ? 1.25 : 1.7));
  renderer.setClearColor(0x000000, 0);

  const group = new THREE.Group();
  scene.add(group);

  const palette = [0x00e0b8, 0xff4f64, 0xffd447, 0x8a63ff, 0xffffff];
  const geometry = new THREE.CylinderGeometry(0.56, 0.56, 0.18, 6, 1, false);
  const edgeGeometry = new THREE.EdgesGeometry(geometry);
  const meshes = [];
  const radius = isMobile ? 3 : 4;

  for (let q = -radius; q <= radius; q += 1) {
    for (let r = -radius; r <= radius; r += 1) {
      const s = -q - r;
      if (Math.max(Math.abs(q), Math.abs(r), Math.abs(s)) > radius) continue;

      const color = palette[Math.abs(q * 2 + r * 3) % palette.length];
      const material = new THREE.MeshStandardMaterial({
        color,
        metalness: 0.24,
        roughness: 0.34,
        emissive: color,
        emissiveIntensity: 0.05,
      });
      const mesh = new THREE.Mesh(geometry, material);
      const x = Math.sqrt(3) * (q + r / 2);
      const y = 1.5 * r;
      mesh.position.set(x, y, Math.sin(q * 1.7 + r * 0.8) * 0.55);
      mesh.rotation.x = Math.PI / 2;
      mesh.userData = {
        homeZ: mesh.position.z,
        phase: (q * 13 + r * 17) * 0.17,
        baseScale: 0.75 + Math.random() * 0.38,
        material,
      };
      mesh.scale.setScalar(mesh.userData.baseScale);

      const edges = new THREE.LineSegments(
        edgeGeometry,
        new THREE.LineBasicMaterial({ color: 0x0b0b0b, transparent: true, opacity: 0.34 }),
      );
      edges.rotation.copy(mesh.rotation);
      mesh.add(edges);

      group.add(mesh);
      meshes.push(mesh);
    }
  }

  group.rotation.x = -0.52;
  group.rotation.z = -0.18;
  group.position.set(isMobile ? 1.1 : 4.3, isMobile ? -0.9 : -0.3, 0);

  const ambient = new THREE.AmbientLight(0xffffff, 0.72);
  const key = new THREE.DirectionalLight(0xffffff, 2.4);
  key.position.set(2, 4, 8);
  const rim = new THREE.PointLight(0x00e0b8, 2.8, 24);
  rim.position.set(-6, -4, 6);
  scene.add(ambient, key, rim);

  const particleGeometry = new THREE.BufferGeometry();
  const particleCount = isMobile ? 110 : 170;
  const positions = new Float32Array(particleCount * 3);

  for (let i = 0; i < particleCount; i += 1) {
    positions[i * 3] = (Math.random() - 0.5) * 18;
    positions[i * 3 + 1] = (Math.random() - 0.5) * 12;
    positions[i * 3 + 2] = -2 - Math.random() * 8;
  }

  particleGeometry.setAttribute('position', new THREE.BufferAttribute(positions, 3));
  const particles = new THREE.Points(
    particleGeometry,
    new THREE.PointsMaterial({
      color: 0xffffff,
      size: isMobile ? 0.025 : 0.035,
      transparent: true,
      opacity: 0.42,
    }),
  );
  scene.add(particles);

  const raycaster = new THREE.Raycaster();
  const pointer = new THREE.Vector2(0.25, 0);
  const targetTilt = new THREE.Vector2(0.25, 0);
  let hovered = null;
  let pulse = 0;
  let raf = 0;

  const resize = () => {
    const rect = hero.getBoundingClientRect();
    renderer.setSize(rect.width, rect.height, false);
    camera.aspect = rect.width / Math.max(rect.height, 1);
    camera.updateProjectionMatrix();
  };

  const updatePointer = (event) => {
    const rect = canvas.getBoundingClientRect();
    pointer.x = ((event.clientX - rect.left) / rect.width) * 2 - 1;
    pointer.y = -(((event.clientY - rect.top) / rect.height) * 2 - 1);
    targetTilt.set(pointer.x, pointer.y);
  };

  const updateHover = () => {
    raycaster.setFromCamera(pointer, camera);
    const hit = raycaster.intersectObjects(meshes, false)[0]?.object ?? null;

    if (hovered && hovered !== hit) {
      hovered.userData.material.emissiveIntensity = 0.05;
    }

    hovered = hit;

    if (hovered) {
      hovered.userData.material.emissiveIntensity = 0.42;
    }
  };

  const animate = (time = 0) => {
    const t = time * 0.001;

    if (!prefersReducedMotion) {
      group.rotation.y += (targetTilt.x * 0.22 - group.rotation.y) * 0.045;
      group.rotation.x += (-0.52 + targetTilt.y * 0.11 - group.rotation.x) * 0.045;
      particles.rotation.z = t * 0.025;
    }

    pulse *= 0.91;
    meshes.forEach((mesh, index) => {
      const lift = prefersReducedMotion ? 0 : Math.sin(t * 1.35 + mesh.userData.phase) * 0.08;
      const hoverLift = mesh === hovered ? 0.42 : 0;
      const clickLift = pulse * Math.sin(index * 0.8 + t * 8) * 0.16;
      mesh.position.z = mesh.userData.homeZ + lift + hoverLift + clickLift;

      const targetScale = mesh.userData.baseScale * (mesh === hovered ? 1.18 : 1);
      mesh.scale.lerp(new THREE.Vector3(targetScale, targetScale, targetScale), 0.11);
    });

    renderer.render(scene, camera);
    raf = window.requestAnimationFrame(animate);
  };

  resize();
  updateHover();
  animate();

  window.addEventListener('resize', resize);
  canvas.addEventListener('pointermove', updatePointer);
  canvas.addEventListener('pointermove', updateHover);
  canvas.addEventListener('pointerleave', () => targetTilt.set(0.18, 0));
  canvas.addEventListener('click', () => {
    pulse = 1;
  });

  document.addEventListener('visibilitychange', () => {
    if (document.hidden) {
      window.cancelAnimationFrame(raf);
    } else {
      animate();
    }
  });
}

initReveal();
initBriefForm();
initHexScene();
