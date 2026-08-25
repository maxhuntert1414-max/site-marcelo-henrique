/**
 * Materiais do santuário. Uma instância por "chave" usada no GeoBatch.
 */
import * as THREE from 'three';
import { createBoneTextures } from './textures.js';

export function createShrineMaterials({ textureSize = 512 } = {}) {
  const bone = createBoneTextures(textureSize);

  const boneMaterial = new THREE.MeshStandardMaterial({
    map: bone.map,
    normalMap: bone.normalMap,
    roughnessMap: bone.roughnessMap,
    color: new THREE.Color(0xdcd0b6),
    roughness: 0.92,
    metalness: 0.02,
    normalScale: new THREE.Vector2(0.75, 0.75),
  });

  const toothMaterial = new THREE.MeshStandardMaterial({
    map: bone.map,
    normalMap: bone.normalMap,
    color: new THREE.Color(0xf3ecdb),
    roughness: 0.42,
    metalness: 0.04,
    normalScale: new THREE.Vector2(0.5, 0.5),
  });

  const roofMaterial = new THREE.MeshStandardMaterial({
    map: bone.map,
    normalMap: bone.normalMap,
    roughnessMap: bone.roughnessMap,
    color: new THREE.Color(0x9c8d74),
    roughness: 0.96,
    metalness: 0.03,
    normalScale: new THREE.Vector2(0.95, 0.95),
  });

  const stoneMaterial = new THREE.MeshStandardMaterial({
    map: bone.map,
    normalMap: bone.normalMap,
    roughnessMap: bone.roughnessMap,
    color: new THREE.Color(0x3d352d),
    roughness: 1,
    metalness: 0.02,
    normalScale: new THREE.Vector2(0.7, 0.7),
  });

  // Órbitas, boca e suturas: breu com energia amaldiçoada vazando
  const voidMaterial = new THREE.MeshStandardMaterial({
    color: new THREE.Color(0x080403),
    emissive: new THREE.Color(0xff2a08),
    emissiveIntensity: 0.05,
    roughness: 1,
    metalness: 0,
    side: THREE.DoubleSide,
  });

  // Suturas, fossas e frestas: sombra seca, sem energia amaldiçoada
  const seamMaterial = new THREE.MeshStandardMaterial({
    color: new THREE.Color(0x241d17),
    roughness: 1,
    metalness: 0,
    side: THREE.DoubleSide,
  });

  const textures = [bone.map, bone.normalMap, bone.roughnessMap];
  textures.forEach((texture) => {
    texture.wrapS = THREE.RepeatWrapping;
    texture.wrapT = THREE.RepeatWrapping;
    texture.repeat.set(2, 2);
  });

  return {
    bone: boneMaterial,
    tooth: toothMaterial,
    roof: roofMaterial,
    stone: stoneMaterial,
    void: voidMaterial,
    seam: seamMaterial,
    dispose() {
      [boneMaterial, toothMaterial, roofMaterial, stoneMaterial, voidMaterial, seamMaterial].forEach((m) => m.dispose());
      textures.forEach((t) => t.dispose());
    },
  };
}
