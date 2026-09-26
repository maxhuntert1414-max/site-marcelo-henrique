package com.marcelohenrique.tecladoremoto

import com.marcelohenrique.tecladoremoto.core.Mods.ALT
import com.marcelohenrique.tecladoremoto.core.Mods.CTRL
import com.marcelohenrique.tecladoremoto.core.Mods.SHIFT
import com.marcelohenrique.tecladoremoto.core.Mods.WIN
import com.marcelohenrique.tecladoremoto.core.Vk

sealed interface KeyAction {
    data class Tap(val vk: Int, val mods: Int = 0) : KeyAction
    object AltTab : KeyAction
    object LockPc : KeyAction
    enum class Clip : KeyAction { PASTE, TYPE, SEND, COPY_FROM_PC, FETCH }
}

class KeyDef(val label: String, val action: KeyAction?, val repeat: Boolean = false, val detail: String? = null)

class Section(val title: String, val columns: Int, val keys: List<KeyDef>)

class Tab(val title: String, val sections: List<Section>, val mouse: Boolean = false)

/** O que aparece em cada aba. Os atalhos são os padrões do Windows e dos navegadores. */
object Panels {
    private val SPACER = KeyDef("", null)

    private fun tap(label: String, vk: Int, mods: Int = 0, repeat: Boolean = false) = KeyDef(label, KeyAction.Tap(vk, mods), repeat)
    private fun ctrl(label: String, letter: Char, repeat: Boolean = false) = tap(label, Vk.letter(letter), CTRL, repeat)
    private fun win(label: String, letter: Char) = tap(label, Vk.letter(letter), WIN)

    val tabs: List<Tab> = listOf(
        Tab(
            "Atalhos",
            listOf(
                Section(
                    "Edição", 4,
                    listOf(
                        ctrl("Copiar", 'c'), ctrl("Colar", 'v'), ctrl("Recortar", 'x'), ctrl("Selec. tudo", 'a'),
                        ctrl("Desfazer", 'z', repeat = true), ctrl("Refazer", 'y', repeat = true),
                        ctrl("Salvar", 's'), ctrl("Localizar", 'f'),
                    ),
                ),
                Section(
                    "Janelas", 4,
                    listOf(
                        KeyDef("Alt+Tab", KeyAction.AltTab), tap("Fechar janela", Vk.f(4), ALT),
                        win("Área de trabalho", 'd'), tap("Ver janelas", Vk.TAB, WIN),
                        tap("Maximizar", Vk.UP, WIN), tap("Minimizar", Vk.DOWN, WIN),
                        tap("Encaixar ◀", Vk.LEFT, WIN), tap("Encaixar ▶", Vk.RIGHT, WIN),
                    ),
                ),
                Section(
                    "Navegador", 4,
                    listOf(
                        ctrl("Nova aba", 't'), ctrl("Fechar aba", 'w'), tap("Reabrir aba", Vk.letter('t'), CTRL or SHIFT),
                        tap("Atualizar", Vk.f(5)),
                        tap("◀ Aba", Vk.TAB, CTRL or SHIFT, repeat = true), tap("Aba ▶", Vk.TAB, CTRL, repeat = true),
                        tap("Voltar", Vk.LEFT, ALT), tap("Avançar", Vk.RIGHT, ALT),
                        tap("Zoom +", Vk.OEM_PLUS, CTRL, repeat = true), tap("Zoom −", Vk.OEM_MINUS, CTRL, repeat = true),
                        ctrl("Endereço", 'l'), tap("Tela cheia", Vk.f(11)),
                    ),
                ),
                Section(
                    "Sistema", 4,
                    listOf(
                        tap("Iniciar", Vk.LWIN), win("Pesquisar", 's'), win("Explorador", 'e'), win("Executar", 'r'),
                        tap("Captura", Vk.letter('s'), WIN or SHIFT), tap("Emojis", Vk.OEM_PERIOD, WIN),
                        tap("Gerenciador", Vk.ESCAPE, CTRL or SHIFT), win("Configurações", 'i'),
                        tap("Renomear", Vk.f(2)), tap("Propriedades", Vk.RETURN, ALT), tap("Menu", Vk.APPS),
                        KeyDef("Bloquear PC", KeyAction.LockPc),
                    ),
                ),
            ),
        ),
        Tab(
            "Teclas",
            listOf(
                Section(
                    "Navegação", 4,
                    listOf(
                        tap("Esc", Vk.ESCAPE), tap("Tab", Vk.TAB, repeat = true), tap("Enter", Vk.RETURN, repeat = true),
                        tap("⌫ Apagar", Vk.BACK, repeat = true),
                        tap("Home", Vk.HOME), tap("End", Vk.END), tap("PgUp", Vk.PRIOR, repeat = true),
                        tap("PgDn", Vk.NEXT, repeat = true),
                        tap("Insert", Vk.INSERT), tap("Delete", Vk.DELETE, repeat = true), tap("Espaço", Vk.SPACE, repeat = true),
                        tap("Caps Lock", Vk.CAPITAL),
                        tap("PrtSc", Vk.SNAPSHOT), tap("Pause", Vk.PAUSE), tap("Num Lock", Vk.NUMLOCK), tap("Scroll Lock", Vk.SCROLL),
                    ),
                ),
                Section(
                    "Setas", 3,
                    listOf(
                        SPACER, tap("▲", Vk.UP, repeat = true), SPACER,
                        tap("◀", Vk.LEFT, repeat = true), tap("▼", Vk.DOWN, repeat = true), tap("▶", Vk.RIGHT, repeat = true),
                    ),
                ),
                Section("Função", 4, (1..12).map { tap("F$it", Vk.f(it), repeat = true) }),
            ),
        ),
        Tab(
            "Mídia",
            listOf(
                Section(
                    "Volume", 3,
                    listOf(
                        tap("🔉 Vol −", Vk.VOLUME_DOWN, repeat = true), tap("🔇 Mudo", Vk.VOLUME_MUTE),
                        tap("🔊 Vol +", Vk.VOLUME_UP, repeat = true),
                    ),
                ),
                Section(
                    "Reprodução", 4,
                    listOf(
                        tap("⏮", Vk.MEDIA_PREV), tap("⏯", Vk.MEDIA_PLAY_PAUSE), tap("⏭", Vk.MEDIA_NEXT), tap("⏹", Vk.MEDIA_STOP),
                    ),
                ),
                Section(
                    "Navegador", 4,
                    listOf(
                        tap("Voltar", Vk.BROWSER_BACK), tap("Avançar", Vk.BROWSER_FORWARD),
                        tap("Recarregar", Vk.BROWSER_REFRESH), tap("Início", Vk.BROWSER_HOME),
                    ),
                ),
            ),
        ),
        Tab(
            "Copiar e colar",
            listOf(
                Section(
                    "Do celular para o PC", 1,
                    listOf(
                        KeyDef("Colar no PC", KeyAction.Clip.PASTE, detail = "Cola onde o cursor do PC está o que você copiou no celular"),
                        KeyDef("Digitar no PC", KeyAction.Clip.TYPE, detail = "Digita o texto copiado, letra por letra (para campos que bloqueiam colar)"),
                        KeyDef("Só enviar", KeyAction.Clip.SEND, detail = "Coloca na área de transferência do PC, para você colar depois"),
                    ),
                ),
                Section(
                    "Do PC para o celular", 1,
                    listOf(
                        KeyDef("Copiar seleção do PC", KeyAction.Clip.COPY_FROM_PC, detail = "Faz Ctrl+C no PC e traz o texto para o celular"),
                        KeyDef("Trazer área de transferência", KeyAction.Clip.FETCH, detail = "Traz o que já está copiado no PC"),
                    ),
                ),
            ),
        ),
        Tab("Mouse", emptyList(), mouse = true),
    )
}
