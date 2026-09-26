# Protocolo TRK1

Protocolo binário entre o app Android (cliente) e o Teclado Remoto do PC
(servidor). Tudo que é inteiro está em **big-endian**. Texto é **UTF-8**.

| Item                 | Valor                 |
|----------------------|-----------------------|
| Porta TCP (sessão)   | 47800 (se ocupada, o PC tenta 47801–47809 e anuncia a porta real) |
| Porta UDP (descoberta) | 47810               |
| Mágica               | `TRK1`                |
| Versão               | 1                     |

## Moldura (framing)

Toda mensagem TCP é `u32 tamanho` + `tamanho` bytes. O handshake aceita no
máximo 512 bytes por moldura; na sessão cifrada o limite é 1 MiB + 16.
Cada moldura é escrita com uma única chamada `write` e `TCP_NODELAY` ligado
nos dois lados (sem atraso de Nagle).

## Handshake

### CLIENT_HELLO (cliente → PC, texto claro)

| Offset | Tam. | Campo |
|-------:|-----:|-------|
| 0   | 4  | `TRK1` |
| 4   | 1  | versão = 1 |
| 5   | 1  | modo: 1 = SESSÃO (já pareado), 2 = PAREAR |
| 6   | 16 | `device_id` (aleatório, fixo por instalação do app) |
| 22  | 65 | chave pública efêmera P-256, SEC1 sem compressão (`04‖X‖Y`) |
| 87  | 32 | `Nc`: SESSÃO = aleatório; PAREAR = zeros (o nonce vai depois) |
| 119 | 1  | tamanho do nome (≤ 64) |
| 120 | n  | nome do celular |

### SERVER_HELLO (PC → cliente, texto claro)

| Offset | Tam. | Campo |
|-------:|-----:|-------|
| 0   | 4  | `TRK1` |
| 4   | 1  | versão |
| 5   | 1  | status (tabela abaixo) |

Se status = 0 continua:

| Offset | Tam. | Campo |
|-------:|-----:|-------|
| 6   | 16 | `server_id` |
| 22  | 65 | chave pública efêmera do PC |
| 87  | 32 | SESSÃO: `Ns` aleatório. PAREAR: compromisso `C = SHA256("TRK1-commit" ‖ pubPC ‖ pubCel ‖ Ns)` |
| 119 | 1  | tamanho do nome |
| 120 | n  | nome do PC |

Status: 0 OK · 1 NÃO_PAREADO · 2 PAREAMENTO_DESATIVADO · 3 OCUPADO ·
4 VERSÃO_INCOMPATÍVEL · 5 RECUSADO · 6 PEDIDO_INVÁLIDO · 7 TEMPO_ESGOTADO.

### Pareamento (comparação numérica com compromisso)

Mesmo esquema do Bluetooth SSP “Numeric Comparison”: o PC se compromete com
`Ns` antes de conhecer `Nc`, então um intermediário não consegue escolher
chaves para forçar códigos iguais (chance de 1 em 1.000.000 por tentativa).

1. cliente → PC: `Nc` (32 bytes)
2. PC → cliente: `Ns` (32 bytes); o cliente confere o compromisso `C`.
3. Os dois mostram o código `SAS = u32(SHA256("TRK1-sas" ‖ pubCel ‖ pubPC ‖ Nc ‖ Ns)[0..4]) mod 1 000 000`.
4. O usuário confirma no PC. PC → cliente: 1 byte (0 = aceito, 5 = recusado, 7 = expirou).
5. Aceito: segue direto para a sessão cifrada (sem reconectar).

Chaves:

```
Z      = ECDH(P-256) — coordenada X (32 bytes)
th     = SHA256(CLIENT_HELLO ‖ SERVER_HELLO ‖ Nc ‖ Ns)          (pareamento)
th     = SHA256(CLIENT_HELLO ‖ SERVER_HELLO)                     (sessão)
K_par  = HKDF-SHA256(salt = th, ikm = Z, info = "TRK1-pair", 32)  (guardada nos dois lados)
OKM    = HKDF-SHA256(salt = K_par, ikm = Z, info = "TRK1-session" ‖ th, 64)
k_c2s  = OKM[0..32]   k_s2c = OKM[32..64]
```

Cada conexão usa chaves efêmeras novas (sigilo futuro). Quem não conhece
`K_par` não consegue abrir nem forjar nenhuma moldura da sessão.

## Sessão cifrada

Moldura = AES-256-GCM(`texto`) ‖ tag de 16 bytes. Nonce de 12 bytes =
`00 00 00 dir` ‖ `u64 contador` (dir 1 = cliente→PC, 2 = PC→cliente; o
contador começa em 0 e sobe a cada moldura). Uma moldura que não abre derruba
a conexão. A primeira moldura do PC é sempre `WELCOME`.

Texto claro = `u8 tipo` ‖ corpo.

### Cliente → PC

| Tipo | Nome | Corpo |
|-----:|------|-------|
| 0x01 | TYPE | `u16 backspaces` ‖ texto — apaga N caracteres e digita o texto (Unicode, independe do layout do PC). `\n` vira Enter, `\t` vira Tab |
| 0x02 | KEY | `u8 ação` (0 toque, 1 segura, 2 solta) ‖ `u8 mods` ‖ `u16 vk` (virtual-key do Windows) |
| 0x03 | CHAR | `u8 mods` ‖ `u32 codepoint` — tecla que produz o caractere no layout do PC, com modificadores (ex.: Ctrl+C) |
| 0x04 | RELEASE_ALL | — solta tudo que estiver pressionado |
| 0x10 | CLIP_SET | texto → área de transferência do PC |
| 0x11 | CLIP_GET | — pede a área de transferência do PC |
| 0x12 | CLIP_PASTE | texto → área de transferência + Ctrl+V |
| 0x20 | MOUSE_MOVE | `i16 dx` ‖ `i16 dy` |
| 0x21 | MOUSE_BUTTON | `u8 botão` (1 esq., 2 dir., 3 meio) ‖ `u8 ação` (0 clique, 1 segura, 2 solta) |
| 0x22 | MOUSE_WHEEL | `i16 vertical` ‖ `i16 horizontal` (120 = um “clique” da roda) |
| 0x30 | ACTION | `u8 id` (1 = bloquear o PC) |
| 0x40 | PING | 8 bytes opacos |
| 0x41 | BYE | — encerra |

Modificadores (`mods`): 1 Ctrl · 2 Shift · 4 Alt · 8 Win.

### PC → cliente

| Tipo | Nome | Corpo |
|-----:|------|-------|
| 0x80 | WELCOME | `u8 flags` (bit 0 = PC rodando como administrador) ‖ nome do PC |
| 0x81 | PONG | os 8 bytes do PING |
| 0x82 | CLIP_DATA | texto da área de transferência do PC |
| 0x83 | NOTICE | `u8 código` (1 = janela ativa é de administrador e bloqueia a digitação) ‖ texto |

O cliente manda PING a cada 2 s. O PC derruba a sessão após 12 s sem nada e,
ao derrubar, **solta todas as teclas e botões** que o celular deixou
pressionados.

## Descoberta (UDP 47810)

* Sonda (broadcast): `TRK1?` ‖ `u32 id` (9 bytes).
* Resposta (unicast para quem perguntou): `TRK1!` ‖ `u32 id` ‖ `u8 versão` ‖
  `server_id (16)` ‖ `u16 porta TCP` ‖ `u8 tamanho` ‖ nome.
