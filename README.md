# Site promocional Marcelo Henrique

Website estático com Vite e Three.js para demonstrar sites, automações e sistemas sob medida.

## Rodar localmente

```powershell
npm install
npm run dev
```

## Gerar build

```powershell
npm run build
```

## Malevolent Shrine (伏魔御廚子)

`malevolent-shrine.html` é uma simulação 3D interativa do Domínio de Ryomen
Sukuna, feita como recriação de fã. Em desenvolvimento ela fica em
`http://127.0.0.1:5173/malevolent-shrine.html`.

Toda a cena é **gerada por código**: não há nenhum modelo, textura ou imagem
externa no repositório. O santuário nasce de primitivas ósseas procedurais
(`src/shrine/bones.js`), montadas em `src/shrine/shrine.js`, com texturas de
ruído fBm criadas em runtime (`src/shrine/textures.js`).

| Arquivo | Papel |
| --- | --- |
| `src/shrine/main.js` | cena, luzes, câmeras, abertura do domínio e interface |
| `src/shrine/shrine.js` | composição do santuário (plataforma, colunata, telhados, crânio, braços) |
| `src/shrine/bones.js` | primitivas: ossos longos, costelas, vértebras, crânio, mãos |
| `src/shrine/textures.js` | texturas procedurais de osso, solo, brasa e corte |
| `src/shrine/effects.js` | céu, chão, brasas, cinza, cortes e cúpula de abertura |
| `src/shrine/materials.js` | materiais compartilhados pelo lote de geometria |

### Controles

Arraste para orbitar, rolagem para aproximar, dois dedos para deslocar.
Teclas `1`–`5` trocam de câmera, `espaço` liga/desliga a órbita automática,
`C` liga/desliga os cortes e `Esc` pula a abertura.

A qualidade (densidade de geometria, partículas, sombras e resolução) é
escolhida automaticamente a partir do tamanho da tela e do número de núcleos.

### Versão em arquivo único

```powershell
npm run build:standalone
```

Gera `dist-standalone/malevolent-shrine.html` com Three.js, os módulos e o CSS
embutidos — um único arquivo que abre direto no navegador, sem servidor.

> Recriação de fã, sem vínculo com os detentores dos direitos de Jujutsu Kaisen.
