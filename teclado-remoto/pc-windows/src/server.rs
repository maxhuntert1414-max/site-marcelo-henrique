//! Servidor TCP: handshake, pareamento e sessão cifrada com o celular.

use std::io::{self, BufReader, Read, Write};
use std::net::{IpAddr, Ipv4Addr, Shutdown, SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicU16, AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use crate::config::{now_unix, Config, Device};
use crate::crypto::{self, Ephemeral, FrameCipher, SessionKeys, DIR_C2S, DIR_S2C};
use crate::input::{Backend, KeyState, Raw, VK_V};
use crate::lock;
use crate::platform;
use crate::proto::*;

const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(8);
const PAIRING_TIMEOUT: Duration = Duration::from_secs(60);
const SESSION_IDLE_TIMEOUT: Duration = Duration::from_secs(12);
const WRITE_TIMEOUT: Duration = Duration::from_secs(5);
const MAX_CONNECTIONS: usize = 16;
/// Eventos por chamada ao SO. Textos enormes (colar/digitar a área de transferência)
/// vão em lotes para não estourar a fila de mensagens do programa que recebe.
const INJECT_BATCH: usize = 400;

pub struct PairRequest {
    pub device_name: String,
    pub addr: IpAddr,
    pub code: u32,
}

pub trait Ui: Send + Sync {
    fn state_changed(&self);
    /// Mostra o pedido de pareamento; a resposta chega pelo canal.
    fn ask_pairing(&self, request: PairRequest) -> mpsc::Receiver<bool>;
    fn cancel_pairing(&self);
    fn paired(&self, device_name: &str);
}

pub struct SessionInfo {
    id: u64,
    pub device_id: [u8; ID_LEN],
    pub device_name: String,
    pub addr: SocketAddr,
    stream: TcpStream,
}

pub struct Shared {
    pub config: Mutex<Config>,
    pub backend: Arc<dyn Backend>,
    pub ui: Arc<dyn Ui>,
    pub sessions: Mutex<Vec<SessionInfo>>,
    pub port: AtomicU16,
    inject_lock: Mutex<()>,
    pairing_busy: AtomicBool,
    next_session: AtomicU64,
    connections: AtomicUsize,
}

impl Shared {
    pub fn new(config: Config, backend: Arc<dyn Backend>, ui: Arc<dyn Ui>) -> Arc<Shared> {
        Arc::new(Shared {
            port: AtomicU16::new(config.port),
            config: Mutex::new(config),
            backend,
            ui,
            sessions: Mutex::new(Vec::new()),
            inject_lock: Mutex::new(()),
            pairing_busy: AtomicBool::new(false),
            next_session: AtomicU64::new(1),
            connections: AtomicUsize::new(0),
        })
    }

    /// Um lote por vez: duas conexões nunca intercalam eventos no meio de um atalho.
    fn inject(&self, events: &[Raw]) {
        let _guard = lock(&self.inject_lock);
        let mut batches = events.chunks(INJECT_BATCH).peekable();
        while let Some(batch) = batches.next() {
            self.backend.send(batch);
            if batches.peek().is_some() {
                thread::sleep(Duration::from_millis(2));
            }
        }
    }

    fn register(&self, device_id: [u8; ID_LEN], name: &str, addr: SocketAddr, stream: &TcpStream) -> io::Result<u64> {
        let id = self.next_session.fetch_add(1, Ordering::Relaxed);
        let stream = stream.try_clone()?;
        let mut sessions = lock(&self.sessions);
        // O mesmo celular reconectando substitui a conexão antiga.
        for old in sessions.iter().filter(|s| s.device_id == device_id) {
            let _ = old.stream.shutdown(Shutdown::Both);
        }
        sessions.retain(|s| s.device_id != device_id);
        sessions.push(SessionInfo { id, device_id, device_name: name.to_string(), addr, stream });
        Ok(id)
    }

    fn unregister(&self, id: u64) {
        lock(&self.sessions).retain(|s| s.id != id);
    }

    #[cfg_attr(not(windows), allow(dead_code))]
    pub fn disconnect_device(&self, device_id: &[u8; ID_LEN]) {
        for s in lock(&self.sessions).iter().filter(|s| &s.device_id == device_id) {
            let _ = s.stream.shutdown(Shutdown::Both);
        }
    }

    /// Derruba todas as conexões e espera elas soltarem as teclas pressionadas.
    #[cfg_attr(not(windows), allow(dead_code))]
    pub fn shutdown(&self) {
        for s in lock(&self.sessions).iter() {
            let _ = s.stream.shutdown(Shutdown::Both);
        }
        let deadline = Instant::now() + Duration::from_millis(500);
        while !lock(&self.sessions).is_empty() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(10));
        }
    }
}

pub fn start(shared: &Arc<Shared>) -> io::Result<u16> {
    let first = shared.port.load(Ordering::Relaxed);
    let mut last_error = io::Error::new(io::ErrorKind::AddrInUse, "sem porta livre");
    for port in first..first.saturating_add(TCP_PORT_FALLBACKS) {
        match TcpListener::bind((Ipv4Addr::UNSPECIFIED, port)) {
            Ok(listener) => {
                let port = listener.local_addr()?.port();
                shared.port.store(port, Ordering::Relaxed);
                let shared = shared.clone();
                thread::Builder::new().name("accept".into()).spawn(move || accept_loop(shared, listener))?;
                crate::log!("aguardando conexões na porta TCP {port}");
                return Ok(port);
            }
            Err(e) => last_error = e,
        }
    }
    Err(last_error)
}

struct ConnectionSlot(Arc<Shared>);

impl Drop for ConnectionSlot {
    fn drop(&mut self) {
        self.0.connections.fetch_sub(1, Ordering::Relaxed);
    }
}

fn accept_loop(shared: Arc<Shared>, listener: TcpListener) {
    for conn in listener.incoming() {
        let stream = match conn {
            Ok(s) => s,
            Err(e) => {
                crate::log!("erro ao aceitar conexão: {e}");
                thread::sleep(Duration::from_millis(100));
                continue;
            }
        };
        let Ok(peer) = stream.peer_addr() else { continue };
        if shared.connections.fetch_add(1, Ordering::Relaxed) >= MAX_CONNECTIONS {
            shared.connections.fetch_sub(1, Ordering::Relaxed);
            continue;
        }
        let slot = ConnectionSlot(shared.clone());
        let spawned = thread::Builder::new().name(format!("conn {peer}")).spawn(move || {
            if let Err(e) = handle_connection(&slot.0, stream, peer) {
                if !is_disconnect(&e) {
                    crate::log!("{peer}: {e}");
                }
            }
        });
        if let Err(e) = spawned {
            crate::log!("não consegui criar thread: {e}");
        }
    }
}

fn is_disconnect(e: &io::Error) -> bool {
    use io::ErrorKind::*;
    matches!(
        e.kind(),
        UnexpectedEof | ConnectionReset | ConnectionAborted | BrokenPipe | TimedOut | WouldBlock | NotConnected
    )
}

fn invalid(msg: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, msg.to_string())
}

fn read_frame(reader: &mut impl Read, max: usize, buf: &mut Vec<u8>) -> io::Result<()> {
    let mut len = [0u8; 4];
    reader.read_exact(&mut len)?;
    let len = u32::from_be_bytes(len) as usize;
    if len == 0 || len > max {
        return Err(invalid("moldura com tamanho inválido"));
    }
    buf.resize(len, 0);
    reader.read_exact(buf)
}

fn write_frame(writer: &mut TcpStream, payload: &[u8]) -> io::Result<()> {
    // Uma única escrita por moldura = um único pacote na rede.
    let mut frame = Vec::with_capacity(4 + payload.len());
    frame.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    frame.extend_from_slice(payload);
    writer.write_all(&frame)
}

fn peer_closed(stream: &TcpStream) -> bool {
    if stream.set_nonblocking(true).is_err() {
        return true;
    }
    let mut probe = [0u8; 1];
    let closed = match stream.peek(&mut probe) {
        Ok(0) => true,
        Ok(_) => false,
        Err(e) => e.kind() != io::ErrorKind::WouldBlock,
    };
    let _ = stream.set_nonblocking(false);
    closed
}

fn handle_connection(shared: &Arc<Shared>, stream: TcpStream, peer: SocketAddr) -> io::Result<()> {
    stream.set_nodelay(true)?;
    stream.set_read_timeout(Some(HANDSHAKE_TIMEOUT))?;
    stream.set_write_timeout(Some(WRITE_TIMEOUT))?;
    let mut writer = stream.try_clone()?;
    let mut reader = BufReader::with_capacity(16 * 1024, stream);

    let mut hello_bytes = Vec::new();
    read_frame(&mut reader, MAX_HANDSHAKE_FRAME, &mut hello_bytes)?;
    let hello = match ClientHello::parse(&hello_bytes) {
        Ok(h) => h,
        Err(HelloError::BadVersion) => return write_frame(&mut writer, &server_status(ST_BAD_VERSION)),
        Err(HelloError::Malformed) => return Ok(()),
    };
    let ephemeral = Ephemeral::generate();
    let Some(shared_secret) = ephemeral.agree(&hello.public) else {
        return write_frame(&mut writer, &server_status(ST_BAD_REQUEST));
    };
    let (server_id, server_name, allow_pairing, known_key) = {
        let cfg = lock(&shared.config);
        let key = cfg.find(&hello.device_id).map(|d| d.key);
        (cfg.server_id, cfg.name.clone(), cfg.allow_pairing, key)
    };

    if hello.mode == MODE_SESSION {
        let Some(pair_key) = known_key else {
            crate::log!("{peer}: \"{}\" não está pareado", hello.name);
            return write_frame(&mut writer, &server_status(ST_NOT_PAIRED));
        };
        let nonce: [u8; NONCE_LEN] = crypto::random_bytes();
        let server_hello = ServerHello {
            server_id: &server_id,
            public: &ephemeral.public,
            nonce_or_commit: &nonce,
            name: &server_name,
        }
        .encode();
        write_frame(&mut writer, &server_hello)?;
        let transcript = crypto::sha256(&[&hello_bytes, &server_hello]);
        let keys = crypto::session_keys(&pair_key, &shared_secret, &transcript);
        return run_session(shared, reader, writer, keys, hello, peer);
    }

    if !allow_pairing {
        return write_frame(&mut writer, &server_status(ST_PAIRING_DISABLED));
    }
    if shared.pairing_busy.swap(true, Ordering::SeqCst) {
        return write_frame(&mut writer, &server_status(ST_BUSY));
    }
    let outcome = {
        let _busy = BusyFlag(&shared.pairing_busy);
        pair(
            shared,
            &mut reader,
            &mut writer,
            &hello,
            &hello_bytes,
            &ephemeral,
            &shared_secret,
            peer,
            &server_id,
            &server_name,
        )
    };
    match outcome? {
        Some(keys) => run_session(shared, reader, writer, keys, hello, peer),
        None => Ok(()),
    }
}

/// Só um pedido de pareamento por vez (evita enxurrada de avisos no PC).
struct BusyFlag<'a>(&'a AtomicBool);

impl Drop for BusyFlag<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

#[allow(clippy::too_many_arguments)]
fn pair(
    shared: &Arc<Shared>,
    reader: &mut BufReader<TcpStream>,
    writer: &mut TcpStream,
    hello: &ClientHello,
    hello_bytes: &[u8],
    ephemeral: &Ephemeral,
    shared_secret: &[u8; 32],
    peer: SocketAddr,
    server_id: &[u8; ID_LEN],
    server_name: &str,
) -> io::Result<Option<SessionKeys>> {
    let server_nonce: [u8; NONCE_LEN] = crypto::random_bytes();
    let commit = crypto::commitment(&ephemeral.public, &hello.public, &server_nonce);
    let server_hello =
        ServerHello { server_id, public: &ephemeral.public, nonce_or_commit: &commit, name: server_name }.encode();
    write_frame(writer, &server_hello)?;

    let mut buf = Vec::new();
    read_frame(reader, NONCE_LEN, &mut buf)?;
    let client_nonce: [u8; NONCE_LEN] = buf.as_slice().try_into().map_err(|_| invalid("nonce inválido"))?;
    write_frame(writer, &server_nonce)?;

    let transcript = crypto::sha256(&[hello_bytes, &server_hello, &client_nonce, &server_nonce]);
    let code = crypto::sas_code(&hello.public, &ephemeral.public, &client_nonce, &server_nonce);
    let pair_key = crypto::pair_key(&transcript, shared_secret);

    crate::log!("{peer}: pedido de pareamento de \"{}\"", hello.name);
    let answer = shared.ui.ask_pairing(PairRequest { device_name: hello.name.clone(), addr: peer.ip(), code });
    let started = Instant::now();
    let decision = loop {
        match answer.recv_timeout(Duration::from_millis(250)) {
            Ok(accepted) => break Some(accepted),
            Err(RecvTimeoutError::Disconnected) => break Some(false),
            Err(RecvTimeoutError::Timeout) => {
                if started.elapsed() > PAIRING_TIMEOUT || peer_closed(reader.get_ref()) {
                    shared.ui.cancel_pairing();
                    break None;
                }
            }
        }
    };
    match decision {
        Some(true) => {}
        Some(false) => {
            crate::log!("{peer}: pareamento recusado");
            write_frame(writer, &[ST_REJECTED])?;
            return Ok(None);
        }
        None => {
            crate::log!("{peer}: pedido de pareamento expirou ou foi cancelado");
            let _ = write_frame(writer, &[ST_TIMEOUT]);
            return Ok(None);
        }
    }

    {
        let mut cfg = lock(&shared.config);
        cfg.upsert(Device {
            id: hello.device_id,
            name: hello.name.clone(),
            key: pair_key,
            paired_at: now_unix(),
            last_seen: now_unix(),
        });
        if let Err(e) = cfg.save() {
            crate::log!("não consegui salvar o pareamento: {e}");
        }
    }
    write_frame(writer, &[ST_OK])?;
    crate::log!("{peer}: \"{}\" pareado", hello.name);
    shared.ui.paired(&hello.name);
    shared.ui.state_changed();
    Ok(Some(crypto::session_keys(&pair_key, shared_secret, &transcript)))
}

/// Solta tudo o que o celular deixou pressionado, mesmo se a thread entrar em pânico.
struct SessionGuard<'a> {
    shared: &'a Shared,
    keys: KeyState,
    session: Option<u64>,
    peer: SocketAddr,
}

impl Drop for SessionGuard<'_> {
    fn drop(&mut self) {
        let mut out = Vec::new();
        self.keys.release_all(&mut out);
        if !out.is_empty() {
            self.shared.inject(&out);
        }
        if let Some(id) = self.session {
            self.shared.unregister(id);
            self.shared.ui.state_changed();
            crate::log!("{}: desconectado", self.peer);
        }
    }
}

const BLOCKED_TEXT: &str = "A janela ativa no PC está rodando como administrador e bloqueia a digitação. \
Abra o Teclado Remoto como administrador para controlar essa janela.";

fn run_session(
    shared: &Arc<Shared>,
    mut reader: BufReader<TcpStream>,
    mut writer: TcpStream,
    keys: SessionKeys,
    hello: ClientHello,
    peer: SocketAddr,
) -> io::Result<()> {
    let mut tx = FrameCipher::new(&keys.s2c, DIR_S2C);
    let mut rx = FrameCipher::new(&keys.c2s, DIR_C2S);
    let backend = shared.backend.clone();
    let flags = if backend.is_elevated() { WELCOME_FLAG_ELEVATED } else { 0 };
    let name = lock(&shared.config).name.clone();
    write_frame(&mut writer, &tx.seal(&welcome(flags, &name)))?;
    reader.get_ref().set_read_timeout(Some(SESSION_IDLE_TIMEOUT))?;

    let mut guard = SessionGuard { shared: shared.as_ref(), keys: KeyState::default(), session: None, peer };
    let mut frame = Vec::with_capacity(1024);
    let mut events = Vec::with_capacity(64);
    let mut warned_blocked = false;
    loop {
        read_frame(&mut reader, MAX_PLAINTEXT + TAG_LEN, &mut frame)?;
        let Some(plain) = rx.open(&frame) else {
            if guard.session.is_none() {
                crate::log!("{peer}: chave de pareamento não confere (\"{}\")", hello.name);
            }
            return Err(invalid("moldura não autenticada"));
        };
        if guard.session.is_none() {
            // Só aqui o celular provou que conhece a chave: registra e assume a sessão.
            guard.session = Some(shared.register(hello.device_id, &hello.name, peer, reader.get_ref())?);
            {
                let mut cfg = lock(&shared.config);
                cfg.touch(&hello.device_id, &hello.name);
                let _ = cfg.save();
            }
            platform::boost_thread_priority();
            shared.ui.state_changed();
            crate::log!("{peer}: conectado (\"{}\")", hello.name);
        }
        let Some(msg) = ClientMsg::parse(&plain) else { continue };

        events.clear();
        let keys = &mut guard.keys;
        let reply = match msg {
            ClientMsg::Type { backspaces, text } => {
                keys.type_text(backspaces, text, &mut events);
                None
            }
            ClientMsg::Key { action, mods, vk } => {
                keys.key(action, mods, vk, &mut events);
                None
            }
            ClientMsg::Char { mods, codepoint } => {
                keys.char_key(mods, codepoint, |c| backend.resolve_char(c), &mut events);
                None
            }
            ClientMsg::ReleaseAll => {
                keys.release_all(&mut events);
                None
            }
            ClientMsg::ClipSet(text) => {
                backend.clipboard_set(text);
                None
            }
            ClientMsg::ClipGet => Some(clip_data(&backend.clipboard_get().unwrap_or_default())),
            ClientMsg::ClipPaste(text) => {
                if backend.clipboard_set(text) {
                    keys.key(KEY_TAP, MOD_CTRL, VK_V, &mut events);
                }
                None
            }
            ClientMsg::MouseMove { dx, dy } => {
                events.push(Raw::MouseMove { dx: dx.into(), dy: dy.into() });
                None
            }
            ClientMsg::MouseButton { button, action } => {
                keys.mouse_button(button, action, &mut events);
                None
            }
            ClientMsg::MouseWheel { vertical, horizontal } => {
                if vertical != 0 {
                    events.push(Raw::Wheel { delta: vertical.into(), horizontal: false });
                }
                if horizontal != 0 {
                    events.push(Raw::Wheel { delta: horizontal.into(), horizontal: true });
                }
                None
            }
            ClientMsg::Action(ACTION_LOCK) => {
                backend.lock_workstation();
                None
            }
            ClientMsg::Ping(payload) => Some(pong(&payload)),
            ClientMsg::Bye => return Ok(()),
            ClientMsg::Action(_) | ClientMsg::Unknown(_) => None,
        };

        if !events.is_empty() {
            let blocked = backend.input_blocked();
            if blocked && !warned_blocked {
                write_frame(&mut writer, &tx.seal(&notice(NOTICE_INPUT_BLOCKED, BLOCKED_TEXT)))?;
            }
            warned_blocked = blocked;
            shared.inject(&events);
        }
        if let Some(reply) = reply {
            write_frame(&mut writer, &tx.seal(&reply))?;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::{VK_BACK, VK_CONTROL, VK_MENU, VK_RETURN};
    use std::path::PathBuf;

    #[derive(Default)]
    struct Recorder {
        events: Mutex<Vec<Raw>>,
        clipboard: Mutex<String>,
    }

    impl Backend for Recorder {
        fn send(&self, inputs: &[Raw]) {
            lock(&self.events).extend_from_slice(inputs);
        }
        fn resolve_char(&self, _: u32) -> Option<(u16, u8)> {
            None
        }
        fn clipboard_get(&self) -> Option<String> {
            Some(lock(&self.clipboard).clone())
        }
        fn clipboard_set(&self, text: &str) -> bool {
            *lock(&self.clipboard) = text.to_string();
            true
        }
        fn lock_workstation(&self) {}
        fn input_blocked(&self) -> bool {
            false
        }
        fn is_elevated(&self) -> bool {
            false
        }
    }

    struct AutoUi(bool);

    impl Ui for AutoUi {
        fn state_changed(&self) {}
        fn ask_pairing(&self, _: PairRequest) -> mpsc::Receiver<bool> {
            let (tx, rx) = mpsc::channel();
            tx.send(self.0).unwrap();
            rx
        }
        fn cancel_pairing(&self) {}
        fn paired(&self, _: &str) {}
    }

    struct TestServer {
        shared: Arc<Shared>,
        recorder: Arc<Recorder>,
        port: u16,
        dir: PathBuf,
    }

    impl Drop for TestServer {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    fn start_server(accept: bool) -> TestServer {
        let dir = std::env::temp_dir().join(format!("trk-srv-{}", crypto::to_hex(&crypto::random_bytes::<6>())));
        let mut cfg = Config::load(&dir, "PC de teste");
        cfg.port = 0;
        let recorder = Arc::new(Recorder::default());
        let shared = Shared::new(cfg, recorder.clone(), Arc::new(AutoUi(accept)));
        let port = start(&shared).unwrap();
        TestServer { shared, recorder, port, dir }
    }

    struct Client {
        stream: TcpStream,
        tx: FrameCipher,
        rx: FrameCipher,
    }

    impl Client {
        fn send(&mut self, plain: &[u8]) {
            let sealed = self.tx.seal(plain);
            write_frame(&mut self.stream, &sealed).unwrap();
        }
        fn recv(&mut self) -> Vec<u8> {
            let mut buf = Vec::new();
            read_frame(&mut self.stream, 1 << 21, &mut buf).unwrap();
            self.rx.open(&buf).expect("moldura do servidor deve abrir")
        }
        /// PING/PONG garante que tudo enviado antes já foi processado.
        fn sync(&mut self) {
            self.send(&[MSG_PING, 1, 2, 3, 4, 5, 6, 7, 8]);
            assert_eq!(self.recv(), vec![MSG_PONG, 1, 2, 3, 4, 5, 6, 7, 8]);
        }
    }

    fn connect(port: u16) -> TcpStream {
        let s = TcpStream::connect(("127.0.0.1", port)).unwrap();
        s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        s
    }

    fn read_plain(stream: &mut TcpStream) -> Vec<u8> {
        let mut buf = Vec::new();
        read_frame(stream, 4096, &mut buf).unwrap();
        buf
    }

    fn hello(mode: u8, device: [u8; 16], eph: &Ephemeral, nonce: [u8; 32]) -> Vec<u8> {
        ClientHello { mode, device_id: device, public: eph.public, nonce, name: "Celular teste".into() }.encode()
    }

    fn pair(port: u16, device: [u8; 16]) -> Result<(Client, [u8; 32]), u8> {
        let mut stream = connect(port);
        let eph = Ephemeral::generate();
        let ch = hello(MODE_PAIR, device, &eph, [0; 32]);
        write_frame(&mut stream, &ch).unwrap();
        let sh = read_plain(&mut stream);
        if sh[5] != ST_OK {
            return Err(sh[5]);
        }
        let server_pub: [u8; 65] = sh[22..87].try_into().unwrap();
        let commit: [u8; 32] = sh[87..119].try_into().unwrap();
        let nc: [u8; 32] = crypto::random_bytes();
        write_frame(&mut stream, &nc).unwrap();
        let ns: [u8; 32] = read_plain(&mut stream).try_into().unwrap();
        assert_eq!(crypto::commitment(&server_pub, &eph.public, &ns), commit);
        let result = read_plain(&mut stream);
        if result != [ST_OK] {
            return Err(result[0]);
        }
        let z = eph.agree(&server_pub).unwrap();
        let th = crypto::sha256(&[&ch, &sh, &nc, &ns]);
        let key = crypto::pair_key(&th, &z);
        let keys = crypto::session_keys(&key, &z, &th);
        let mut client =
            Client { stream, tx: FrameCipher::new(&keys.c2s, DIR_C2S), rx: FrameCipher::new(&keys.s2c, DIR_S2C) };
        let welcome = client.recv();
        assert_eq!(welcome[0], MSG_WELCOME);
        assert_eq!(&welcome[2..], "PC de teste".as_bytes());
        Ok((client, key))
    }

    fn session(port: u16, device: [u8; 16], key: &[u8; 32]) -> Result<Client, u8> {
        let mut stream = connect(port);
        let eph = Ephemeral::generate();
        let ch = hello(MODE_SESSION, device, &eph, crypto::random_bytes());
        write_frame(&mut stream, &ch).unwrap();
        let sh = read_plain(&mut stream);
        if sh[5] != ST_OK {
            return Err(sh[5]);
        }
        let z = eph.agree(sh[22..87].try_into().unwrap()).unwrap();
        let keys = crypto::session_keys(key, &z, &crypto::sha256(&[&ch, &sh]));
        let mut client =
            Client { stream, tx: FrameCipher::new(&keys.c2s, DIR_C2S), rx: FrameCipher::new(&keys.s2c, DIR_S2C) };
        let mut buf = Vec::new();
        read_frame(&mut client.stream, 4096, &mut buf).unwrap();
        match client.rx.open(&buf) {
            Some(w) if w[0] == MSG_WELCOME => Ok(client),
            _ => Err(255),
        }
    }

    fn wait_for(recorder: &Recorder, event: Raw) {
        let deadline = Instant::now() + Duration::from_secs(2);
        while !lock(&recorder.events).contains(&event) {
            assert!(Instant::now() < deadline, "evento {event:?} não chegou");
            thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    fn pairing_then_session_types_and_releases_keys() {
        let server = start_server(true);
        let device = [3u8; 16];
        let (mut client, key) = pair(server.port, device).unwrap();
        assert_eq!(lock(&server.shared.config).find(&device).unwrap().key, key);

        let mut type_msg = vec![MSG_TYPE, 0, 1];
        type_msg.extend_from_slice("é\n".as_bytes());
        client.send(&type_msg);
        client.send(&[MSG_KEY, KEY_TAP, MOD_CTRL, 0, 0x43]);
        client.send(&[MSG_KEY, KEY_DOWN, 0, 0, VK_MENU as u8]);
        client.sync();
        assert_eq!(lock(&server.shared.sessions).len(), 1);
        {
            let events = lock(&server.recorder.events);
            assert_eq!(
                *events,
                vec![
                    Raw::Key { vk: VK_BACK, up: false },
                    Raw::Key { vk: VK_BACK, up: true },
                    Raw::Unicode { unit: 0xE9, up: false },
                    Raw::Unicode { unit: 0xE9, up: true },
                    Raw::Key { vk: VK_RETURN, up: false },
                    Raw::Key { vk: VK_RETURN, up: true },
                    Raw::Key { vk: VK_CONTROL, up: false },
                    Raw::Key { vk: 0x43, up: false },
                    Raw::Key { vk: 0x43, up: true },
                    Raw::Key { vk: VK_CONTROL, up: true },
                    Raw::Key { vk: VK_MENU, up: false },
                ]
            );
        }
        drop(client);
        wait_for(&server.recorder, Raw::Key { vk: VK_MENU, up: true });

        let mut client = session(server.port, device, &key).unwrap();
        let mut clip = vec![MSG_CLIP_SET];
        clip.extend_from_slice("copiado do celular ✓".as_bytes());
        client.send(&clip);
        client.send(&[MSG_CLIP_GET]);
        let data = client.recv();
        assert_eq!(data[0], MSG_CLIP_DATA);
        assert_eq!(std::str::from_utf8(&data[1..]).unwrap(), "copiado do celular ✓");
    }

    #[test]
    fn reconnect_replaces_old_session() {
        let server = start_server(true);
        let device = [4u8; 16];
        let (mut first, key) = pair(server.port, device).unwrap();
        first.sync();
        let mut second = session(server.port, device, &key).unwrap();
        second.sync();
        assert_eq!(lock(&server.shared.sessions).len(), 1);
        let mut buf = Vec::new();
        assert!(read_frame(&mut first.stream, 4096, &mut buf).is_err(), "sessão antiga deve cair");
    }

    #[test]
    fn unknown_device_wrong_key_and_rejection_are_refused() {
        let server = start_server(true);
        assert_eq!(session(server.port, [9; 16], &[0; 32]).err(), Some(ST_NOT_PAIRED));

        let (_, _key) = pair(server.port, [5; 16]).unwrap();
        // Chave errada: o WELCOME não abre e nada é injetado.
        assert_eq!(session(server.port, [5; 16], &[1; 32]).err(), Some(255));

        let rejecting = start_server(false);
        assert_eq!(pair(rejecting.port, [6; 16]).err(), Some(ST_REJECTED));
        assert!(lock(&rejecting.shared.config).find(&[6; 16]).is_none());

        lock(&server.shared.config).allow_pairing = false;
        assert_eq!(pair(server.port, [7; 16]).err(), Some(ST_PAIRING_DISABLED));
    }

    #[test]
    fn forged_session_cannot_kick_real_one() {
        let server = start_server(true);
        let device = [8u8; 16];
        let (mut real, _key) = pair(server.port, device).unwrap();
        real.sync();
        // Alguém que só sabe o device_id (vai em texto claro) conecta e manda lixo.
        let mut stream = connect(server.port);
        let eph = Ephemeral::generate();
        write_frame(&mut stream, &hello(MODE_SESSION, device, &eph, [1; 32])).unwrap();
        let _ = read_plain(&mut stream);
        let _ = read_plain(&mut stream);
        write_frame(&mut stream, &[0u8; 40]).unwrap();
        let mut buf = Vec::new();
        assert!(read_frame(&mut stream, 4096, &mut buf).is_err());
        real.sync();
        assert_eq!(lock(&server.shared.sessions).len(), 1);
    }
}
