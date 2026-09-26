package com.marcelohenrique.tecladoremoto

import android.annotation.SuppressLint
import android.app.Activity
import android.app.AlertDialog
import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.content.res.Configuration
import android.graphics.Typeface
import android.graphics.drawable.GradientDrawable
import android.net.wifi.WifiManager
import android.os.Build
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.text.InputType
import android.text.SpannableStringBuilder
import android.text.Spanned
import android.text.style.ForegroundColorSpan
import android.text.style.RelativeSizeSpan
import android.util.TypedValue
import android.view.Gravity
import android.view.HapticFeedbackConstants
import android.view.KeyEvent
import android.view.MotionEvent
import android.view.View
import android.view.WindowInsets
import android.view.WindowManager
import android.view.inputmethod.InputMethodManager
import android.widget.Button
import android.widget.EditText
import android.widget.LinearLayout
import android.widget.PopupMenu
import android.widget.ProgressBar
import android.widget.ScrollView
import android.widget.Space
import android.widget.TextView
import android.widget.Toast
import com.marcelohenrique.tecladoremoto.core.Crypto
import com.marcelohenrique.tecladoremoto.core.Discovery
import com.marcelohenrique.tecladoremoto.core.FoundServer
import com.marcelohenrique.tecladoremoto.core.Mods
import com.marcelohenrique.tecladoremoto.core.Proto
import com.marcelohenrique.tecladoremoto.core.ProtocolException
import com.marcelohenrique.tecladoremoto.core.Session
import com.marcelohenrique.tecladoremoto.core.Vk
import java.io.IOException
import java.net.InetAddress
import java.net.Socket
import java.util.concurrent.atomic.AtomicReference

class MainActivity : Activity(), RemoteClient.Listener, CaptureEditText.Sink {
    private lateinit var store: ServerStore
    private lateinit var client: RemoteClient
    private lateinit var modifiers: Modifiers
    private val handler = Handler(Looper.getMainLooper())

    private lateinit var root: View
    private lateinit var screenConnect: View
    private lateinit var screenKeyboard: View
    private lateinit var serverList: LinearLayout
    private lateinit var emptyHint: TextView
    private lateinit var scanProgress: ProgressBar
    private lateinit var statusDot: View
    private lateinit var serverName: TextView
    private lateinit var latency: TextView
    private lateinit var tabsRow: LinearLayout
    private lateinit var panel: LinearLayout
    private lateinit var panelScroll: ScrollView
    private lateinit var mousePanel: View
    private lateinit var quickRow: LinearLayout
    private lateinit var modeButton: Button
    private lateinit var sendButton: Button
    private lateinit var input: CaptureEditText

    private var started = false
    private var currentServer: SavedServer? = null
    private var selectedTab = 0
    private val modifierButtons = LinkedHashMap<Int, Button>()
    private var altTabButton: Button? = null
    private var altHeld = false
    private var imeVisible = false
    private var wifiLock: WifiManager.WifiLock? = null
    private var backToken: Any? = null
    private var scanning = false
    private val discovered = LinkedHashMap<String, FoundServer>()
    private var lastOfflineWarning = 0L
    private var heldMouseMods = emptyList<Int>()

    private val onKeyboard get() = currentServer != null

    private val releaseAlt = Runnable {
        if (altHeld) {
            altHeld = false
            client.send(Proto.key(Proto.KEY_UP, 0, Vk.MENU))
            renderAltTab()
        }
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_main)
        store = ServerStore(this)
        client = RemoteClient(store, this)
        modifiers = Modifiers(::renderModifiers)
        bindViews()
        setupEdgeToEdge()
        setupConnectScreen()
        setupKeyboardScreen()
        val last = store.lastServerId?.let(store::find)
        if (last != null) showKeyboard(last) else showConnect()
    }

    override fun onStart() {
        super.onStart()
        started = true
        currentServer?.let {
            client.start(it)
            acquireWifiLock()
        }
        if (!onKeyboard) startScan()
    }

    override fun onStop() {
        started = false
        handler.removeCallbacks(releaseAlt)
        altHeld = false
        client.stop()
        releaseWifiLock()
        super.onStop()
    }

    override fun onConfigurationChanged(newConfig: Configuration) {
        super.onConfigurationChanged(newConfig)
        if (onKeyboard) selectTab(selectedTab)
    }

    private fun bindViews() {
        root = findViewById(R.id.root)
        screenConnect = findViewById(R.id.screen_connect)
        screenKeyboard = findViewById(R.id.screen_keyboard)
        serverList = findViewById(R.id.server_list)
        emptyHint = findViewById(R.id.empty_hint)
        scanProgress = findViewById(R.id.scan_progress)
        statusDot = findViewById(R.id.status_dot)
        serverName = findViewById(R.id.server_name)
        latency = findViewById(R.id.latency)
        tabsRow = findViewById(R.id.tabs)
        panel = findViewById(R.id.panel)
        panelScroll = findViewById(R.id.panel_scroll)
        mousePanel = findViewById(R.id.mouse_panel)
        quickRow = findViewById(R.id.quick_row)
        modeButton = findViewById(R.id.mode_button)
        sendButton = findViewById(R.id.send_button)
        input = findViewById(R.id.input)
    }

    // ---------------------------------------------------------------- janela

    private fun setupEdgeToEdge() {
        if (Build.VERSION.SDK_INT >= 35) {
            // Android 15+ já desenha de ponta a ponta para apps com targetSdk 35+.
        } else if (Build.VERSION.SDK_INT >= 30) {
            @Suppress("DEPRECATION")
            window.setDecorFitsSystemWindows(false)
        } else {
            @Suppress("DEPRECATION")
            window.decorView.systemUiVisibility = View.SYSTEM_UI_FLAG_LAYOUT_STABLE or
                View.SYSTEM_UI_FLAG_LAYOUT_HIDE_NAVIGATION or View.SYSTEM_UI_FLAG_LAYOUT_FULLSCREEN
        }
        // Barra de status, barra de navegação e teclado do celular viram margem do conteúdo.
        root.setOnApplyWindowInsetsListener { view, insets ->
            if (Build.VERSION.SDK_INT >= 30) {
                val bars = insets.getInsets(WindowInsets.Type.systemBars() or WindowInsets.Type.displayCutout())
                val ime = insets.getInsets(WindowInsets.Type.ime())
                imeVisible = insets.isVisible(WindowInsets.Type.ime())
                view.setPadding(bars.left, bars.top, bars.right, maxOf(bars.bottom, ime.bottom))
            } else {
                @Suppress("DEPRECATION")
                view.setPadding(
                    insets.systemWindowInsetLeft,
                    insets.systemWindowInsetTop,
                    insets.systemWindowInsetRight,
                    insets.systemWindowInsetBottom,
                )
                @Suppress("DEPRECATION")
                imeVisible = insets.systemWindowInsetBottom > dp(120)
            }
            insets
        }
    }

    private fun dp(value: Int) = (value * resources.displayMetrics.density).toInt()

    private fun color(id: Int) = if (Build.VERSION.SDK_INT >= 23) getColor(id) else @Suppress("DEPRECATION") resources.getColor(id)

    private fun toast(text: String, long: Boolean = false) =
        Toast.makeText(this, text, if (long) Toast.LENGTH_LONG else Toast.LENGTH_SHORT).show()

    private fun dialog() = AlertDialog.Builder(this, android.R.style.Theme_Material_Dialog_Alert)

    private fun haptic(view: View) {
        if (store.haptics) view.performHapticFeedback(HapticFeedbackConstants.KEYBOARD_TAP)
    }

    // ---------------------------------------------------------------- tela de conexão

    private fun setupConnectScreen() {
        findViewById<Button>(R.id.scan_button).setOnClickListener { startScan() }
        findViewById<Button>(R.id.manual_button).setOnClickListener { askAddress() }
    }

    private fun showConnect() {
        currentServer = null
        handler.removeCallbacks(releaseAlt)
        altHeld = false
        client.stop()
        releaseWifiLock()
        hideIme()
        window.clearFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
        backToken?.let { BackCompat.unregister(this, it) }
        backToken = null
        screenKeyboard.visibility = View.GONE
        screenConnect.visibility = View.VISIBLE
        renderServers()
        if (started) startScan()
    }

    private fun startScan() {
        if (scanning) return
        scanning = true
        discovered.clear()
        scanProgress.visibility = View.VISIBLE
        renderServers()
        Thread({
            try {
                Discovery.scan(timeoutMs = 1600) { found ->
                    runOnUiThread {
                        discovered[found.idHex] = found
                        renderServers()
                    }
                }
            } catch (_: Exception) {
            }
            runOnUiThread {
                scanning = false
                scanProgress.visibility = View.GONE
                renderServers()
            }
        }, "descoberta").start()
    }

    private fun renderServers() {
        serverList.removeAllViews()
        val saved = store.servers()
        for (s in saved) {
            val found = discovered[s.id] ?: continue
            val updated = SavedServer(s.id, found.name, found.host, found.port, s.key)
            addServerCard(found.name, "${found.host}  ·  pareado", onClick = { showKeyboard(updated) }, onForget = { forget(s) })
        }
        for (found in discovered.values) {
            if (saved.any { it.id == found.idHex }) continue
            addServerCard(found.name, "${found.host}  ·  toque para parear", onClick = { pair(found.host, found.port, found.name) })
        }
        for (s in saved) {
            if (discovered.containsKey(s.id)) continue
            val status = if (scanning) "procurando…" else "não encontrado agora (último IP ${s.host})"
            addServerCard(s.name, "pareado  ·  $status", onClick = { showKeyboard(s) }, onForget = { forget(s) })
        }
        emptyHint.visibility = if (serverList.childCount == 0 && !scanning) View.VISIBLE else View.GONE
    }

    private fun addServerCard(title: String, subtitle: String, onClick: () -> Unit, onForget: (() -> Unit)? = null) {
        val card = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setBackgroundResource(R.drawable.card_bg)
            setPadding(dp(16), dp(14), dp(16), dp(14))
            isClickable = true
            setOnClickListener {
                haptic(it)
                onClick()
            }
            if (onForget != null) {
                setOnLongClickListener {
                    onForget()
                    true
                }
            }
        }
        card.addView(TextView(this).apply {
            text = title
            setTextColor(color(R.color.text))
            textSize = 17f
            typeface = Typeface.create("sans-serif-medium", Typeface.NORMAL)
        })
        card.addView(TextView(this).apply {
            text = subtitle
            setTextColor(color(R.color.text_dim))
            textSize = 13f
        })
        serverList.addView(card, LinearLayout.LayoutParams(-1, -2).apply { bottomMargin = dp(10) })
    }

    private fun forget(server: SavedServer) {
        dialog()
            .setTitle("Esquecer ${server.name}?")
            .setMessage("Para usar esse PC de novo será preciso parear outra vez. (No PC, você pode remover este celular na janela do Teclado Remoto.)")
            .setPositiveButton("Esquecer") { _, _ ->
                store.remove(server.id)
                renderServers()
            }
            .setNegativeButton("Cancelar", null)
            .show()
    }

    private fun askAddress() {
        val field = EditText(this).apply {
            hint = "192.168.0.10"
            inputType = InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_VARIATION_URI
            setSingleLine()
        }
        val box = LinearLayout(this).apply {
            setPadding(dp(20), dp(8), dp(20), 0)
            addView(field, LinearLayout.LayoutParams(-1, -2))
        }
        dialog()
            .setTitle("Endereço do PC")
            .setMessage("Aparece na janela do Teclado Remoto no computador.")
            .setView(box)
            .setPositiveButton("Conectar") { _, _ -> connectToAddress(field.text.toString().trim()) }
            .setNegativeButton("Cancelar", null)
            .show()
    }

    private fun connectToAddress(address: String) {
        if (address.isEmpty()) return
        val host = address.substringBefore(':')
        val port = address.substringAfter(':', "").toIntOrNull() ?: Proto.TCP_PORT
        Thread({
            // Pergunta direto ao IP quem ele é (se o UDP passar); senão tenta parear na porta TCP.
            var found: FoundServer? = null
            try {
                Discovery.scan(timeoutMs = 700, targets = listOf(InetAddress.getByName(host))) { if (found == null) found = it }
            } catch (_: Exception) {
            }
            runOnUiThread {
                val f = found
                val saved = f?.let { store.find(it.idHex) }
                when {
                    saved != null -> showKeyboard(SavedServer(saved.id, f.name, f.host, f.port, saved.key))
                    f != null -> pair(f.host, f.port, f.name)
                    else -> pair(host, port, host)
                }
            }
        }, "endereco").start()
    }

    private fun pair(host: String, port: Int, name: String) {
        stopScanUi()
        val status = TextView(this).apply {
            text = "Conectando a $name…"
            setTextColor(color(R.color.text_dim))
            textSize = 15f
        }
        val code = TextView(this).apply {
            setTextColor(color(R.color.text))
            textSize = 40f
            typeface = Typeface.MONOSPACE
            gravity = Gravity.CENTER
            visibility = View.GONE
        }
        val box = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(dp(24), dp(12), dp(24), dp(8))
            addView(status)
            addView(code, LinearLayout.LayoutParams(-1, -2).apply { topMargin = dp(16) })
        }
        val socket = AtomicReference<Socket?>()
        var cancelled = false
        val cancel = {
            cancelled = true
            Thread { socket.get()?.close() }.start()
        }
        val progress = dialog()
            .setTitle("Parear com $name")
            .setView(box)
            .setNegativeButton("Cancelar") { _, _ -> cancel() }
            .setOnCancelListener { cancel() }
            .show()
        Thread({
            try {
                val result = Session.pair(
                    host, port, store.identity,
                    onCode = { sas ->
                        runOnUiThread {
                            status.text = "Confira na tela do computador: o código precisa ser igual a este. Se for, clique em Permitir no PC."
                            code.text = sas
                            code.visibility = View.VISIBLE
                        }
                    },
                    socketReady = { socket.set(it) },
                )
                val session = result.session
                val server = SavedServer(Crypto.hex(session.serverId), session.serverName.ifEmpty { name }, host, port, result.pairKey)
                store.save(server)
                runOnUiThread {
                    progress.dismiss()
                    if (cancelled || isFinishing) {
                        session.close()
                    } else {
                        toast("Pareado com ${server.name}!")
                        showKeyboard(server, session)
                    }
                }
            } catch (e: IOException) {
                runOnUiThread {
                    progress.dismiss()
                    if (!cancelled) showError("Não deu para parear", friendly(e, host, port))
                }
            }
        }, "pareamento").start()
    }

    private fun stopScanUi() {
        scanProgress.visibility = View.GONE
    }

    private fun friendly(e: IOException, host: String, port: Int): String = when (e) {
        is ProtocolException -> e.message.orEmpty()
        else -> "Não consegui falar com $host (porta $port).\n\n" +
            "• O Teclado Remoto está aberto no PC?\n" +
            "• Celular e PC estão na mesma rede Wi-Fi?\n" +
            "• No PC, clique em “Liberar no firewall” na janela do Teclado Remoto."
    }

    private fun showError(title: String, message: String) {
        dialog().setTitle(title).setMessage(message).setPositiveButton("OK", null).show()
    }

    // ---------------------------------------------------------------- tela do teclado

    private fun setupKeyboardScreen() {
        input.sink = this
        applyInputMode(store.inputMode)
        modeButton.setOnClickListener {
            haptic(it)
            val next = (store.inputMode + 1) % CaptureEditText.Mode.values().size
            store.inputMode = next
            applyInputMode(next)
            toast(MODE_HELP[next], long = true)
        }
        sendButton.setOnClickListener {
            haptic(it)
            input.submit(pressEnter = false)
        }
        findViewById<Button>(R.id.ime_button).setOnClickListener {
            haptic(it)
            if (imeVisible) hideIme() else showIme()
        }
        findViewById<Button>(R.id.menu_button).setOnClickListener { showMenu(it) }
        setupQuickRow()
        setupMouse()
        selectTab(0)
    }

    private fun applyInputMode(index: Int) {
        val mode = CaptureEditText.Mode.values()[index.coerceIn(0, MODE_LABELS.size - 1)]
        input.mode = mode
        modeButton.text = MODE_LABELS[mode.ordinal]
        sendButton.visibility = if (mode == CaptureEditText.Mode.COMPOSE) View.VISIBLE else View.GONE
    }

    private fun showKeyboard(server: SavedServer, established: Session? = null) {
        currentServer = server
        store.lastServerId = server.id
        screenConnect.visibility = View.GONE
        screenKeyboard.visibility = View.VISIBLE
        serverName.text = server.name
        latency.text = ""
        setDot(R.color.wait)
        modifiers.clear()
        applyKeepScreenOn()
        if (backToken == null && Build.VERSION.SDK_INT >= 33) backToken = BackCompat.register(this) { showConnect() }
        input.resetBuffer()
        if (started) {
            client.start(server, established)
            acquireWifiLock()
        } else {
            established?.close()
        }
        if (!Panels.tabs[selectedTab].mouse) showIme()
    }

    private fun applyKeepScreenOn() {
        if (onKeyboard && store.keepScreenOn) {
            window.addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
        } else {
            window.clearFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
        }
    }

    private fun setDot(colorId: Int) {
        (statusDot.background.mutate() as GradientDrawable).setColor(color(colorId))
    }

    private fun showIme() {
        input.requestFocus()
        handler.postDelayed({
            if (Build.VERSION.SDK_INT >= 30) {
                input.windowInsetsController?.show(WindowInsets.Type.ime())
            } else {
                (getSystemService(Context.INPUT_METHOD_SERVICE) as InputMethodManager).showSoftInput(input, 0)
            }
        }, 80)
    }

    private fun hideIme() {
        (getSystemService(Context.INPUT_METHOD_SERVICE) as InputMethodManager).hideSoftInputFromWindow(input.windowToken, 0)
    }

    private fun acquireWifiLock() {
        if (wifiLock?.isHeld == true) return
        // Sem isso o Wi-Fi do celular entra em economia de energia e atrasa pacotes em até ~100 ms.
        val wifi = applicationContext.getSystemService(Context.WIFI_SERVICE) as? WifiManager ?: return
        @Suppress("DEPRECATION")
        val mode = if (Build.VERSION.SDK_INT >= 29) WifiManager.WIFI_MODE_FULL_LOW_LATENCY else WifiManager.WIFI_MODE_FULL_HIGH_PERF
        wifiLock = try {
            wifi.createWifiLock(mode, "TecladoRemoto:baixa-latencia").apply {
                setReferenceCounted(false)
                acquire()
            }
        } catch (_: Exception) {
            null
        }
    }

    private fun releaseWifiLock() {
        wifiLock?.let { if (it.isHeld) it.release() }
        wifiLock = null
    }

    // ---------------------------------------------------------------- abas e painéis

    private fun selectTab(index: Int) {
        selectedTab = index
        tabsRow.removeAllViews()
        Panels.tabs.forEachIndexed { i, tab ->
            val b = Button(this, null, 0, R.style.Key_Plain)
            b.text = tab.title
            b.setTextColor(color(if (i == index) R.color.accent else R.color.text_dim))
            b.setOnClickListener {
                haptic(it)
                selectTab(i)
            }
            tabsRow.addView(b, LinearLayout.LayoutParams(-2, dp(44)))
        }
        val tab = Panels.tabs[index]
        if (tab.mouse) {
            panelScroll.visibility = View.GONE
            mousePanel.visibility = View.VISIBLE
            hideIme()
        } else {
            mousePanel.visibility = View.GONE
            panelScroll.visibility = View.VISIBLE
            renderPanel(tab)
        }
    }

    private fun renderPanel(tab: Tab) {
        panel.removeAllViews()
        altTabButton = null
        val wide = resources.configuration.screenWidthDp >= 560
        for (section in tab.sections) {
            panel.addView(TextView(this, null, 0, R.style.Section).apply { text = section.title }, LinearLayout.LayoutParams(-1, -2).apply {
                topMargin = dp(12)
                bottomMargin = dp(4)
                marginStart = dp(4)
            })
            val columns = if (wide && section.columns > 1 && section.keys.size > section.columns) section.columns * 3 / 2 else section.columns
            for (row in section.keys.chunked(columns)) {
                val line = LinearLayout(this).apply { orientation = LinearLayout.HORIZONTAL }
                for (key in row) {
                    val height = if (key.detail != null) -2 else dp(48)
                    val params = LinearLayout.LayoutParams(0, height, 1f).apply { setMargins(dp(3), dp(3), dp(3), dp(3)) }
                    line.addView(if (key.action == null) Space(this) else keyButton(key), params)
                }
                repeat(columns - row.size) { line.addView(Space(this), LinearLayout.LayoutParams(0, 1, 1f)) }
                panel.addView(line, LinearLayout.LayoutParams(-1, -2))
            }
        }
        renderAltTab()
        panelScroll.scrollTo(0, 0)
    }

    // Toque segurado repete/segura a tecla; o OnClickListener cobre o TalkBack (duplo toque).
    @SuppressLint("ClickableViewAccessibility")
    private fun keyButton(key: KeyDef): Button {
        val b = Button(this, null, 0, R.style.Key)
        val action = key.action ?: return b
        if (key.detail != null) {
            val text = SpannableStringBuilder(key.label).append('\n')
            val start = text.length
            text.append(key.detail)
            text.setSpan(RelativeSizeSpan(0.85f), start, text.length, Spanned.SPAN_EXCLUSIVE_EXCLUSIVE)
            text.setSpan(ForegroundColorSpan(color(R.color.text_dim)), start, text.length, Spanned.SPAN_EXCLUSIVE_EXCLUSIVE)
            b.text = text
            b.maxLines = 4
            b.gravity = Gravity.START or Gravity.CENTER_VERTICAL
            b.setPadding(dp(16), dp(12), dp(16), dp(12))
            b.textSize = 15f
        } else {
            b.text = key.label
        }
        if (action == KeyAction.AltTab) altTabButton = b
        if (key.repeat) {
            b.setOnTouchListener(RepeatListener { view ->
                haptic(view)
                perform(action)
            })
        }
        b.setOnClickListener {
            haptic(it)
            perform(action)
        }
        return b
    }

    private fun perform(action: KeyAction) {
        when (action) {
            is KeyAction.Tap -> sendKey(action.vk, action.mods)
            KeyAction.AltTab -> altTab()
            KeyAction.LockPc -> send(Proto.action(Proto.ACTION_LOCK))
            is KeyAction.Clip -> clipboardAction(action)
        }
    }

    // Toque segurado repete/segura a tecla; o OnClickListener cobre o TalkBack (duplo toque).
    @SuppressLint("ClickableViewAccessibility")
    private fun setupQuickRow() {
        val labels = listOf(Mods.CTRL to "Ctrl", Mods.SHIFT to "⇧", Mods.ALT to "Alt", Mods.WIN to "⊞")
        for ((mod, label) in labels) {
            val b = quickButton(label)
            b.setOnClickListener {
                haptic(it)
                modifiers.tap(mod)
            }
            b.setOnLongClickListener {
                haptic(it)
                modifiers.toggleLock(mod)
                true
            }
            modifierButtons[mod] = b
        }
        val keys = listOf("Esc" to Vk.ESCAPE, "Tab" to Vk.TAB, "◀" to Vk.LEFT, "▲" to Vk.UP, "▼" to Vk.DOWN, "▶" to Vk.RIGHT)
        for ((label, vk) in keys) {
            val b = quickButton(label)
            b.setOnTouchListener(RepeatListener { view ->
                haptic(view)
                sendKey(vk)
            })
            b.setOnClickListener { sendKey(vk) }
        }
        renderModifiers()
    }

    private fun quickButton(label: String): Button {
        val b = Button(this, null, 0, R.style.Key)
        b.text = label
        b.textSize = 13f
        b.minHeight = dp(42)
        b.setPadding(0, 0, 0, 0)
        quickRow.addView(b, LinearLayout.LayoutParams(0, dp(44), 1f).apply { setMargins(dp(2), dp(2), dp(2), dp(2)) })
        return b
    }

    private fun renderModifiers() {
        for ((mod, button) in modifierButtons) {
            val state = modifiers.state(mod)
            button.setBackgroundResource(
                when (state) {
                    Modifiers.State.OFF -> R.drawable.key_bg
                    Modifiers.State.ONCE -> R.drawable.key_bg_once
                    Modifiers.State.LOCKED -> R.drawable.key_bg_locked
                },
            )
            button.setTextColor(color(if (state == Modifiers.State.OFF) R.color.text else android.R.color.white))
        }
    }

    private fun renderAltTab() {
        val b = altTabButton ?: return
        b.text = if (altHeld) "Próxima ▸" else "Alt+Tab"
        b.setBackgroundResource(if (altHeld) R.drawable.key_bg_locked else R.drawable.key_bg)
    }

    // Toque segurado repete/segura a tecla; o OnClickListener cobre o TalkBack (duplo toque).
    @SuppressLint("ClickableViewAccessibility")
    private fun setupMouse() {
        findViewById<TouchpadView>(R.id.touchpad).listener = object : TouchpadView.Listener {
            override fun onPointerMove(dx: Int, dy: Int) {
                client.send(Proto.mouseMove(dx, dy))
            }

            override fun onScroll(vertical: Int, horizontal: Int) {
                client.send(Proto.mouseWheel(vertical, horizontal))
            }

            override fun onClick(button: Int) {
                val mods = Modifiers.virtualKeys(modifiers.active)
                mods.forEach { send(Proto.key(Proto.KEY_DOWN, 0, it)) }
                send(Proto.mouseButton(button, Proto.KEY_TAP))
                mods.asReversed().forEach { send(Proto.key(Proto.KEY_UP, 0, it)) }
                modifiers.consumeOnce()
            }
        }
        val row = findViewById<LinearLayout>(R.id.mouse_buttons)
        val buttons = listOf(R.string.mouse_left to Proto.BUTTON_LEFT, R.string.mouse_middle to Proto.BUTTON_MIDDLE, R.string.mouse_right to Proto.BUTTON_RIGHT)
        for ((label, button) in buttons) {
            val b = Button(this, null, 0, R.style.Key)
            b.setText(label)
            // Segurar o botão segura o clique: dá para arrastar com outro dedo no touchpad.
            b.setOnTouchListener { view, event ->
                when (event.actionMasked) {
                    MotionEvent.ACTION_DOWN -> {
                        view.drawableHotspotChanged(event.x, event.y)
                        view.isPressed = true
                        haptic(view)
                        heldMouseMods = Modifiers.virtualKeys(modifiers.active)
                        heldMouseMods.forEach { send(Proto.key(Proto.KEY_DOWN, 0, it)) }
                        send(Proto.mouseButton(button, Proto.KEY_DOWN))
                    }
                    MotionEvent.ACTION_UP, MotionEvent.ACTION_CANCEL -> {
                        view.isPressed = false
                        send(Proto.mouseButton(button, Proto.KEY_UP))
                        heldMouseMods.asReversed().forEach { send(Proto.key(Proto.KEY_UP, 0, it)) }
                        heldMouseMods = emptyList()
                        modifiers.consumeOnce()
                    }
                }
                true
            }
            b.setOnClickListener { send(Proto.mouseButton(button, Proto.KEY_TAP)) }
            row.addView(b, LinearLayout.LayoutParams(0, -1, if (button == Proto.BUTTON_MIDDLE) 0.6f else 1f).apply { setMargins(dp(3), 0, dp(3), 0) })
        }
    }

    private fun showMenu(anchor: View) {
        val popup = PopupMenu(this, anchor)
        val menu = popup.menu
        menu.add(0, MENU_SWITCH, 0, "Trocar de computador")
        menu.add(0, MENU_RELEASE, 1, "Soltar todas as teclas")
        menu.add(0, MENU_SCREEN, 2, "Manter a tela ligada").setCheckable(true).setChecked(store.keepScreenOn)
        menu.add(0, MENU_HAPTICS, 3, "Vibrar ao tocar").setCheckable(true).setChecked(store.haptics)
        menu.add(0, MENU_VOLUME, 4, "Botões de volume controlam o PC").setCheckable(true).setChecked(store.volumeKeys)
        menu.add(0, MENU_HELP, 5, "Como usar")
        popup.setOnMenuItemClickListener { item ->
            when (item.itemId) {
                MENU_SWITCH -> showConnect()
                MENU_RELEASE -> {
                    handler.removeCallbacks(releaseAlt)
                    altHeld = false
                    renderAltTab()
                    modifiers.clear()
                    send(Proto.releaseAll())
                    toast("Todas as teclas foram soltas no PC.")
                }
                MENU_SCREEN -> {
                    store.keepScreenOn = !store.keepScreenOn
                    applyKeepScreenOn()
                }
                MENU_HAPTICS -> store.haptics = !store.haptics
                MENU_VOLUME -> store.volumeKeys = !store.volumeKeys
                MENU_HELP -> dialog().setTitle("Como usar").setMessage(HELP).setPositiveButton("Entendi", null).show()
            }
            true
        }
        popup.show()
    }

    // ---------------------------------------------------------------- envio

    private fun send(message: ByteArray): Boolean {
        val ok = client.send(message)
        if (!ok) {
            val now = System.currentTimeMillis()
            if (now - lastOfflineWarning > 3000) {
                lastOfflineWarning = now
                toast("Sem conexão com o PC agora.")
            }
        }
        return ok
    }

    private fun sendKey(vk: Int, extra: Int = 0) {
        send(Proto.key(Proto.KEY_TAP, extra or modifiers.active, vk))
        modifiers.consumeOnce()
        if (altHeld) {
            handler.removeCallbacks(releaseAlt)
            // Enter/Esc escolhem/cancelam no Alt+Tab; setas continuam navegando.
            handler.postDelayed(releaseAlt, if (vk == Vk.RETURN || vk == Vk.ESCAPE) 0 else ALT_TAB_HOLD_MS)
        }
    }

    private fun altTab() {
        if (!altHeld) {
            if (!send(Proto.key(Proto.KEY_DOWN, 0, Vk.MENU))) return
            altHeld = true
        }
        // Com Shift fixado, anda para trás.
        send(Proto.key(Proto.KEY_TAP, modifiers.active and Mods.SHIFT, Vk.TAB))
        modifiers.consumeOnce()
        handler.removeCallbacks(releaseAlt)
        handler.postDelayed(releaseAlt, ALT_TAB_HOLD_MS)
        renderAltTab()
    }

    private fun phoneClipboard(): String? {
        val manager = getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
        val clip = manager.primaryClip ?: return null
        if (clip.itemCount == 0) return null
        return clip.getItemAt(0).coerceToText(this)?.toString()?.takeIf { it.isNotEmpty() }
    }

    private fun clipboardAction(action: KeyAction.Clip) {
        when (action) {
            KeyAction.Clip.COPY_FROM_PC -> {
                sendKey(Vk.letter('c'), Mods.CTRL)
                handler.postDelayed({ send(Proto.clipGet()) }, 250)
            }
            KeyAction.Clip.FETCH -> send(Proto.clipGet())
            else -> {
                val text = phoneClipboard()
                if (text == null) {
                    toast("A área de transferência do celular está vazia.")
                    return
                }
                when (action) {
                    KeyAction.Clip.PASTE -> send(Proto.clipPaste(text))
                    KeyAction.Clip.TYPE -> send(Proto.type(0, text))
                    else -> if (send(Proto.clipSet(text))) toast("Enviado para a área de transferência do PC.")
                }
            }
        }
    }

    override fun dispatchKeyEvent(event: KeyEvent): Boolean {
        // Até o Android 12 o "voltar" chega como tecla; do 13 em diante o BackCompat cuida disso.
        if (Build.VERSION.SDK_INT < 33 && onKeyboard && event.keyCode == KeyEvent.KEYCODE_BACK) {
            if (event.action == KeyEvent.ACTION_UP && !event.isCanceled) showConnect()
            return true
        }
        if (onKeyboard && store.volumeKeys && client.connected) {
            val vk = when (event.keyCode) {
                KeyEvent.KEYCODE_VOLUME_UP -> Vk.VOLUME_UP
                KeyEvent.KEYCODE_VOLUME_DOWN -> Vk.VOLUME_DOWN
                else -> 0
            }
            if (vk != 0) {
                if (event.action == KeyEvent.ACTION_DOWN) client.send(Proto.key(Proto.KEY_TAP, 0, vk))
                return true
            }
        }
        return super.dispatchKeyEvent(event)
    }

    // ---------------------------------------------------------------- CaptureEditText.Sink

    override fun stickyModifiers() = modifiers.active

    override fun onEdit(backspaces: Int, text: String) {
        send(Proto.type(backspaces, text))
    }

    override fun onKey(vk: Int, mods: Int) = sendKey(vk, mods)

    override fun onChar(codepoint: Int, mods: Int) {
        send(Proto.char(mods or modifiers.active, codepoint))
        modifiers.consumeOnce()
    }

    override fun onSubmit(text: String, pressEnter: Boolean) {
        if (text.isNotEmpty()) send(Proto.type(0, text))
        if (pressEnter) sendKey(Vk.RETURN)
    }

    // ---------------------------------------------------------------- RemoteClient.Listener

    override fun onState(state: RemoteClient.State, detail: String) {
        when (state) {
            RemoteClient.State.CONNECTING -> {
                setDot(R.color.wait)
                latency.text = "conectando…"
            }
            RemoteClient.State.CONNECTED -> {
                setDot(R.color.ok)
                if (detail.isNotEmpty()) serverName.text = detail
                latency.text = ""
                altHeld = false
                renderAltTab()
                // O cursor do PC pode ter mudado enquanto estava desconectado.
                input.resetBuffer()
            }
            RemoteClient.State.OFFLINE -> {
                setDot(R.color.error)
                latency.text = detail
            }
            RemoteClient.State.NEEDS_PAIRING -> {
                setDot(R.color.error)
                latency.text = ""
                val server = currentServer ?: return
                dialog()
                    .setTitle("Parear de novo")
                    .setMessage("O computador ${server.name} não reconhece mais este celular (o pareamento foi removido no PC). Quer parear de novo?")
                    .setPositiveButton("Parear") { _, _ -> pair(server.host, server.port, server.name) }
                    .setNegativeButton("Voltar") { _, _ -> showConnect() }
                    .setCancelable(false)
                    .show()
            }
        }
    }

    override fun onLatency(ms: Int) {
        latency.text = "$ms ms"
    }

    override fun onClipboard(text: String) {
        if (text.isEmpty()) {
            toast("A área de transferência do PC está vazia.")
            return
        }
        val manager = getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
        manager.setPrimaryClip(ClipData.newPlainText("Teclado Remoto", text))
        // No Android 13+ o próprio sistema já avisa que algo foi copiado.
        if (Build.VERSION.SDK_INT < 33) toast("Copiado do PC para o celular.")
    }

    override fun onNotice(text: String) = toast(text, long = true)

    private companion object {
        const val ALT_TAB_HOLD_MS = 1300L
        const val MENU_SWITCH = 1
        const val MENU_RELEASE = 2
        const val MENU_SCREEN = 3
        const val MENU_HAPTICS = 4
        const val MENU_VOLUME = 5
        const val MENU_HELP = 6

        val MODE_LABELS = listOf("Ao vivo", "Direto", "Enviar ⏎")
        val MODE_HELP = listOf(
            "Ao vivo: cada letra aparece no PC na hora, com as sugestões e a correção do seu teclado.",
            "Direto: sem sugestões nem correção — cada tecla vai exatamente como foi digitada (senhas, código, jogos).",
            "Escrever e enviar: escreva a frase inteira aqui e toque em Enviar (ou Enter) para mandar tudo de uma vez.",
        )
        const val HELP = "• Ao vivo: o texto aparece no PC enquanto você digita, com sugestões e correção.\n" +
            "• Direto: cada tecla vai exatamente como foi digitada (bom para senhas, código e jogos).\n" +
            "• Enviar ⏎: escreve a frase no celular e manda de uma vez.\n\n" +
            "• Ctrl, ⇧ (Shift), Alt e ⊞ (Windows): um toque vale para a próxima tecla; dois toques (ou segurar) travam.\n" +
            "  Ex.: toque em Ctrl e depois digite c = Ctrl+C.\n" +
            "• Alt+Tab: toque de novo para ir para a próxima janela; espere um instante e ela abre.\n" +
            "• Segure setas, Apagar e volume para repetir.\n\n" +
            "• Mouse: arraste para mover, toque para clicar, dois dedos para rolar e toque com dois dedos para o clique direito. " +
            "Segure o botão Esquerdo e arraste no touchpad para selecionar ou mover.\n\n" +
            "• Janela de administrador no PC (Gerenciador de Tarefas, instaladores): rode o Teclado Remoto do PC como administrador."
    }
}
