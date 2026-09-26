//! Janela principal, ícone na bandeja, aviso de pareamento e área de transferência.

use std::cell::{Cell, RefCell};
use std::ffi::c_void;
use std::mem::{size_of, zeroed};
use std::ptr::{copy_nonoverlapping, null, null_mut};
use std::sync::atomic::{AtomicBool, AtomicPtr, AtomicU32, Ordering};
use std::sync::{mpsc, Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

use windows_sys::core::PCWSTR;
use windows_sys::Win32::Foundation::{
    GlobalFree, COLORREF, FILETIME, HWND, LPARAM, LRESULT, POINT, RECT, SYSTEMTIME, S_OK, WPARAM,
};
use windows_sys::Win32::Graphics::Gdi::{
    CreateFontW, DeleteObject, GetSysColor, GetSysColorBrush, InvalidateRect, SetBkColor, SetTextColor,
    CLEARTYPE_QUALITY, CLIP_DEFAULT_PRECIS, COLOR_WINDOW, COLOR_WINDOWTEXT, DEFAULT_CHARSET, DEFAULT_PITCH,
    FF_DONTCARE, FW_NORMAL, FW_SEMIBOLD, HDC, HFONT, OUT_DEFAULT_PRECIS,
};
use windows_sys::Win32::Storage::FileSystem::FileTimeToLocalFileTime;
use windows_sys::Win32::System::Com::{CoInitializeEx, COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE};
use windows_sys::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, GetClipboardData, IsClipboardFormatAvailable, OpenClipboard, SetClipboardData,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock, GMEM_MOVEABLE};
use windows_sys::Win32::System::Ole::CF_UNICODETEXT;
use windows_sys::Win32::System::Time::FileTimeToSystemTime;
use windows_sys::Win32::UI::Controls::{
    LoadIconMetric, TaskDialogIndirect, BST_CHECKED, LIM_LARGE, LIM_SMALL, TASKDIALOGCONFIG, TASKDIALOG_BUTTON,
    TDE_FOOTER, TDF_ALLOW_DIALOG_CANCELLATION, TDF_CALLBACK_TIMER, TDM_CLICK_BUTTON, TDM_SET_ELEMENT_TEXT, TDN_CREATED,
    TDN_TIMER,
};
use windows_sys::Win32::UI::HiDpi::{AdjustWindowRectExForDpi, GetDpiForWindow};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::EnableWindow;
use windows_sys::Win32::UI::Shell::{
    Shell_NotifyIconW, NIF_ICON, NIF_INFO, NIF_MESSAGE, NIF_TIP, NIIF_INFO, NIM_ADD, NIM_DELETE, NIM_MODIFY,
    NOTIFYICONDATAW,
};
use windows_sys::Win32::UI::WindowsAndMessaging::*;

use super::sys::{self, wide};
use crate::crypto::format_sas;
use crate::lock;
use crate::server::{PairRequest, Shared, Ui};

const CLASS: &str = "TecladoRemoto.Janela";
const WM_APP_TRAY: u32 = WM_APP + 1;
const WM_APP_STATE: u32 = WM_APP + 2;
const WM_APP_SHOW: u32 = WM_APP + 3;
const WM_APP_CLIP_SET: u32 = WM_APP + 4;
const WM_APP_CLIP_GET: u32 = WM_APP + 5;
const WM_APP_BALLOON: u32 = WM_APP + 6;
const TIMER_ADDRESSES: usize = 1;

const ICON_ACTIVE: u16 = 1;
const ICON_IDLE: u16 = 2;

const GREEN: COLORREF = 0x004A_A316;
const GRAY: COLORREF = 0x0069_5547;
const RED: COLORREF = 0x0026_26DC;
const HINT: COLORREF = 0x0064_6464;

const C_TITLE: usize = 0;
const C_STATUS: usize = 1;
const C_PC: usize = 2;
const C_ADDR: usize = 3;
const C_HELP: usize = 4;
const C_DEVLIST: usize = 6;
const C_REMOVE: usize = 7;
const C_AUTOSTART: usize = 8;
const C_ALLOWPAIR: usize = 9;
const C_FIREWALL: usize = 10;
const C_ADMIN: usize = 11;
const C_QUIT: usize = 12;
const CONTROLS: usize = 13;
const ID_BASE: usize = 100;

const ID_MENU_OPEN: usize = 200;
const ID_MENU_AUTOSTART: usize = 201;
const ID_MENU_ALLOW: usize = 202;
const ID_MENU_QUIT: usize = 203;
const IDCANCEL: usize = 2;

const PAIR_ALLOW: i32 = 1001;
const PAIR_DENY: i32 = 1002;
const PAIR_SECONDS: u64 = 60;

const CLIENT_W: i32 = 460;
const CLIENT_H: i32 = 474;
const WINDOW_STYLE: WINDOW_STYLE = WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX;

struct Spec {
    class: &'static str,
    style: u32,
    text: &'static str,
    rect: (i32, i32, i32, i32),
}

const SS_NOPREFIX: u32 = 0x80;
const STATIC: u32 = SS_NOPREFIX;
const BUTTON: u32 = BS_PUSHBUTTON as u32 | WS_TABSTOP;
const CHECK: u32 = BS_AUTOCHECKBOX as u32 | WS_TABSTOP;

/// Posições em pixels a 96 DPI; tudo é escalado para a tela atual.
const SPECS: [Spec; CONTROLS] = [
    Spec { class: "STATIC", style: STATIC, text: "Teclado Remoto", rect: (20, 14, 420, 34) },
    Spec { class: "STATIC", style: STATIC, text: "", rect: (20, 52, 420, 24) },
    Spec { class: "STATIC", style: STATIC, text: "", rect: (20, 84, 420, 20) },
    Spec { class: "STATIC", style: STATIC, text: "", rect: (20, 106, 420, 20) },
    Spec {
        class: "STATIC",
        style: STATIC,
        text: "No celular, abra o app Teclado Remoto e escolha este PC na lista. \
               Os dois precisam estar na mesma rede Wi-Fi.",
        rect: (20, 134, 420, 40),
    },
    Spec { class: "STATIC", style: STATIC, text: "Celulares pareados", rect: (20, 186, 420, 20) },
    Spec {
        class: "LISTBOX",
        style: WS_BORDER | WS_VSCROLL | WS_TABSTOP | LBS_NOTIFY as u32 | LBS_NOINTEGRALHEIGHT as u32,
        text: "",
        rect: (20, 208, 420, 100),
    },
    Spec { class: "BUTTON", style: BUTTON, text: "Remover selecionado", rect: (20, 316, 170, 30) },
    Spec { class: "BUTTON", style: CHECK, text: "Iniciar junto com o Windows", rect: (20, 360, 420, 24) },
    Spec { class: "BUTTON", style: CHECK, text: "Permitir parear novos celulares", rect: (20, 386, 420, 24) },
    Spec { class: "BUTTON", style: BUTTON, text: "Liberar no firewall", rect: (20, 428, 140, 30) },
    Spec { class: "BUTTON", style: BUTTON, text: "Rodar como administrador", rect: (168, 428, 190, 30) },
    Spec { class: "BUTTON", style: BUTTON, text: "Encerrar", rect: (366, 428, 74, 30) },
];

struct App {
    shared: Arc<Shared>,
    port_ok: bool,
    elevated: bool,
}

static APP: OnceLock<App> = OnceLock::new();
static MAIN: AtomicPtr<c_void> = AtomicPtr::new(null_mut());
static TASKBAR_CREATED: AtomicU32 = AtomicU32::new(0);
static BALLOON: Mutex<Option<(String, String)>> = Mutex::new(None);
static PAIR_CANCEL: Mutex<Option<Arc<AtomicBool>>> = Mutex::new(None);

/// Estado que só a thread da interface usa (Cells: sem empréstimos longos durante chamadas à API).
struct UiState {
    controls: [Cell<HWND>; CONTROLS],
    fonts: [Cell<HFONT>; 3],
    dpi: Cell<u32>,
    status_color: Cell<COLORREF>,
    icons: [Cell<HICON>; 2],
    close_hint_shown: Cell<bool>,
    device_ids: RefCell<Vec<[u8; 16]>>,
    addresses: RefCell<String>,
}

thread_local! {
    static UI: UiState = UiState {
        controls: std::array::from_fn(|_| Cell::new(null_mut())),
        fonts: std::array::from_fn(|_| Cell::new(null_mut())),
        dpi: Cell::new(96),
        status_color: Cell::new(GRAY),
        icons: std::array::from_fn(|_| Cell::new(null_mut())),
        close_hint_shown: Cell::new(false),
        device_ids: RefCell::new(Vec::new()),
        addresses: RefCell::new(String::new()),
    };
}

fn control(index: usize) -> HWND {
    UI.with(|ui| ui.controls[index].get())
}

fn scale(value: i32, dpi: u32) -> i32 {
    (value as i64 * dpi as i64 / 96) as i32
}

fn resource(id: u16) -> PCWSTR {
    id as usize as PCWSTR
}

pub struct WinUi;

fn post(msg: u32) {
    let hwnd = MAIN.load(Ordering::Acquire);
    if !hwnd.is_null() {
        unsafe {
            PostMessageW(hwnd, msg, 0, 0);
        }
    }
}

impl Ui for WinUi {
    fn state_changed(&self) {
        post(WM_APP_STATE);
    }

    fn ask_pairing(&self, request: PairRequest) -> mpsc::Receiver<bool> {
        let (tx, rx) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        *lock(&PAIR_CANCEL) = Some(cancel.clone());
        let spawned = thread::Builder::new().name("pareamento".into()).spawn(move || {
            let _ = tx.send(show_pairing_dialog(&request, &cancel));
        });
        if let Err(e) = spawned {
            crate::log!("não consegui mostrar o pedido de pareamento: {e}");
        }
        rx
    }

    fn cancel_pairing(&self) {
        if let Some(flag) = lock(&PAIR_CANCEL).take() {
            flag.store(true, Ordering::SeqCst);
        }
    }

    fn paired(&self, device_name: &str) {
        *lock(&BALLOON) = Some(("Celular pareado".into(), format!("{device_name} já pode digitar neste PC.")));
        post(WM_APP_BALLOON);
    }
}

struct PairDialog {
    cancel: Arc<AtomicBool>,
    started: Instant,
    seconds_shown: Cell<u64>,
    footer: RefCell<Vec<u16>>,
}

fn show_pairing_dialog(request: &PairRequest, cancel: &Arc<AtomicBool>) -> bool {
    let title = wide("Teclado Remoto");
    let main = wide(&format!("Parear o celular \"{}\"?", request.device_name));
    let content = wide(&format!(
        "Código de verificação:   {}\n\nConfira se o mesmo código aparece na tela do celular. \
         Se for diferente, recuse.\n\nEndereço do celular: {}",
        format_sas(request.code),
        request.addr
    ));
    let allow = wide("Permitir");
    let deny = wide("Recusar");
    let buttons = [
        TASKDIALOG_BUTTON { nButtonID: PAIR_ALLOW, pszButtonText: allow.as_ptr() },
        TASKDIALOG_BUTTON { nButtonID: PAIR_DENY, pszButtonText: deny.as_ptr() },
    ];
    let dialog = PairDialog {
        cancel: cancel.clone(),
        started: Instant::now(),
        seconds_shown: Cell::new(PAIR_SECONDS),
        footer: RefCell::new(wide(&format!("Este pedido expira em {PAIR_SECONDS} s."))),
    };
    let mut pressed = 0;
    let hr = unsafe {
        let mut config: TASKDIALOGCONFIG = zeroed();
        config.cbSize = size_of::<TASKDIALOGCONFIG>() as u32;
        config.hInstance = GetModuleHandleW(null());
        config.dwFlags = TDF_ALLOW_DIALOG_CANCELLATION | TDF_CALLBACK_TIMER;
        config.pszWindowTitle = title.as_ptr();
        config.Anonymous1.pszMainIcon = resource(ICON_ACTIVE);
        config.pszMainInstruction = main.as_ptr();
        config.pszContent = content.as_ptr();
        config.cButtons = buttons.len() as u32;
        config.pButtons = buttons.as_ptr();
        config.nDefaultButton = PAIR_DENY;
        config.pszFooter = dialog.footer.borrow().as_ptr();
        config.pfCallback = Some(pairing_callback);
        config.lpCallbackData = &dialog as *const PairDialog as isize;
        TaskDialogIndirect(&config, &mut pressed, null_mut(), null_mut())
    };
    hr == S_OK && pressed == PAIR_ALLOW && !cancel.load(Ordering::SeqCst)
}

unsafe extern "system" fn pairing_callback(hwnd: HWND, msg: u32, _: WPARAM, _: LPARAM, data: isize) -> i32 {
    let dialog = &*(data as *const PairDialog);
    match msg as i32 {
        TDN_CREATED => {
            SetWindowPos(hwnd, HWND_TOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE);
            SetForegroundWindow(hwnd);
        }
        TDN_TIMER => {
            if dialog.cancel.load(Ordering::SeqCst) {
                SendMessageW(hwnd, TDM_CLICK_BUTTON as u32, PAIR_DENY as WPARAM, 0);
            } else {
                let left = PAIR_SECONDS.saturating_sub(dialog.started.elapsed().as_secs());
                if left != dialog.seconds_shown.get() {
                    dialog.seconds_shown.set(left);
                    *dialog.footer.borrow_mut() = wide(&format!("Este pedido expira em {left} s."));
                    let text = dialog.footer.borrow().as_ptr();
                    SendMessageW(hwnd, TDM_SET_ELEMENT_TEXT as u32, TDE_FOOTER as WPARAM, text as LPARAM);
                }
            }
        }
        _ => {}
    }
    S_OK
}

pub fn clipboard_set(text: &str) -> bool {
    let hwnd = MAIN.load(Ordering::Acquire);
    if hwnd.is_null() {
        return false;
    }
    let text: Vec<u16> = text.encode_utf16().chain(Some(0)).collect();
    // A área de transferência pertence à janela; a thread dela faz o trabalho.
    unsafe { SendMessageW(hwnd, WM_APP_CLIP_SET, &text as *const Vec<u16> as WPARAM, 0) != 0 }
}

pub fn clipboard_get() -> Option<String> {
    let hwnd = MAIN.load(Ordering::Acquire);
    if hwnd.is_null() {
        return None;
    }
    let mut out: Option<String> = None;
    unsafe {
        SendMessageW(hwnd, WM_APP_CLIP_GET, &mut out as *mut Option<String> as WPARAM, 0);
    }
    out
}

unsafe fn open_clipboard(hwnd: HWND) -> bool {
    for _ in 0..10 {
        if OpenClipboard(hwnd) != 0 {
            return true;
        }
        thread::sleep(Duration::from_millis(10));
    }
    false
}

unsafe fn write_clipboard(hwnd: HWND, text: &[u16]) -> bool {
    if !open_clipboard(hwnd) {
        return false;
    }
    EmptyClipboard();
    let mut ok = false;
    let memory = GlobalAlloc(GMEM_MOVEABLE, text.len() * 2);
    if !memory.is_null() {
        let target = GlobalLock(memory) as *mut u16;
        if !target.is_null() {
            copy_nonoverlapping(text.as_ptr(), target, text.len());
            GlobalUnlock(memory);
            ok = !SetClipboardData(CF_UNICODETEXT as u32, memory).is_null();
        }
        if !ok {
            GlobalFree(memory);
        }
    }
    CloseClipboard();
    ok
}

unsafe fn read_clipboard(hwnd: HWND) -> Option<String> {
    if IsClipboardFormatAvailable(CF_UNICODETEXT as u32) == 0 {
        return Some(String::new());
    }
    if !open_clipboard(hwnd) {
        return None;
    }
    let mut result = None;
    let memory = GetClipboardData(CF_UNICODETEXT as u32);
    if !memory.is_null() {
        let source = GlobalLock(memory) as *const u16;
        if !source.is_null() {
            let max = GlobalSize(memory) / 2;
            let slice = std::slice::from_raw_parts(source, max);
            let len = slice.iter().position(|&c| c == 0).unwrap_or(max);
            result = Some(String::from_utf16_lossy(&slice[..len]));
            GlobalUnlock(memory);
        }
    }
    CloseClipboard();
    result
}

/// Outra cópia foi aberta: mostra a janela desta.
pub fn activate_existing() {
    unsafe {
        let hwnd = FindWindowW(wide(CLASS).as_ptr(), null());
        if !hwnd.is_null() {
            let mut pid = 0;
            GetWindowThreadProcessId(hwnd, &mut pid);
            AllowSetForegroundWindow(pid);
            PostMessageW(hwnd, WM_APP_SHOW, 0, 0);
        }
    }
}

pub fn run(shared: Arc<Shared>, port_ok: bool, start_hidden: bool) {
    unsafe {
        CoInitializeEx(null(), (COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE) as u32);
        let instance = GetModuleHandleW(null());
        let _ = APP.set(App { shared, port_ok, elevated: sys::is_elevated() });
        TASKBAR_CREATED.store(RegisterWindowMessageW(wide("TaskbarCreated").as_ptr()), Ordering::Relaxed);

        let mut small: HICON = null_mut();
        let mut large: HICON = null_mut();
        LoadIconMetric(instance, resource(ICON_ACTIVE), LIM_SMALL, &mut small);
        LoadIconMetric(instance, resource(ICON_ACTIVE), LIM_LARGE, &mut large);
        UI.with(|ui| {
            ui.icons[0].set(small);
            let mut idle: HICON = null_mut();
            LoadIconMetric(instance, resource(ICON_IDLE), LIM_SMALL, &mut idle);
            ui.icons[1].set(if idle.is_null() { small } else { idle });
        });

        let class = wide(CLASS);
        let wc = WNDCLASSEXW {
            cbSize: size_of::<WNDCLASSEXW>() as u32,
            style: 0,
            lpfnWndProc: Some(wndproc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: instance,
            hIcon: large,
            hCursor: LoadCursorW(null_mut(), IDC_ARROW),
            hbrBackground: (COLOR_WINDOW + 1) as usize as _,
            lpszMenuName: null(),
            lpszClassName: class.as_ptr(),
            hIconSm: small,
        };
        RegisterClassExW(&wc);
        let hwnd = CreateWindowExW(
            0,
            class.as_ptr(),
            wide("Teclado Remoto").as_ptr(),
            WINDOW_STYLE,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CLIENT_W,
            CLIENT_H,
            null_mut(),
            null_mut(),
            instance,
            null(),
        );
        if hwnd.is_null() {
            sys::message("Não consegui abrir a janela do Teclado Remoto.", true);
            return;
        }
        MAIN.store(hwnd, Ordering::Release);
        add_tray(hwnd);
        refresh_addresses();
        refresh(hwnd);
        if !start_hidden {
            ShowWindow(hwnd, SW_SHOWNORMAL);
            SetForegroundWindow(hwnd);
        }

        let mut msg: MSG = zeroed();
        while GetMessageW(&mut msg, null_mut(), 0, 0) > 0 {
            if IsDialogMessageW(hwnd, &msg) == 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
        MAIN.store(null_mut(), Ordering::Release);
    }
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_CREATE => {
            on_create(hwnd);
            0
        }
        WM_COMMAND => {
            on_command(hwnd, wparam & 0xFFFF, ((wparam >> 16) & 0xFFFF) as u32);
            0
        }
        WM_CTLCOLORSTATIC => on_ctlcolor(wparam as HDC, lparam as HWND),
        WM_DPICHANGED => {
            let r = &*(lparam as *const RECT);
            SetWindowPos(
                hwnd,
                null_mut(),
                r.left,
                r.top,
                r.right - r.left,
                r.bottom - r.top,
                SWP_NOZORDER | SWP_NOACTIVATE,
            );
            apply_dpi((wparam & 0xFFFF) as u32);
            0
        }
        WM_CLOSE => {
            ShowWindow(hwnd, SW_HIDE);
            let first = UI.with(|ui| !ui.close_hint_shown.replace(true));
            if first {
                balloon(hwnd, "Continuo aqui", "O Teclado Remoto segue ativo na bandeja, perto do relógio.");
            }
            0
        }
        WM_DESTROY => {
            KillTimer(hwnd, TIMER_ADDRESSES);
            Shell_NotifyIconW(NIM_DELETE, &tray_data(hwnd));
            PostQuitMessage(0);
            0
        }
        WM_TIMER => {
            refresh_addresses();
            refresh(hwnd);
            0
        }
        WM_APP_TRAY => {
            match (lparam & 0xFFFF) as u32 {
                WM_LBUTTONUP => show(hwnd),
                WM_RBUTTONUP | WM_CONTEXTMENU => tray_menu(hwnd),
                _ => {}
            }
            0
        }
        WM_APP_STATE => {
            refresh(hwnd);
            0
        }
        WM_APP_SHOW => {
            show(hwnd);
            0
        }
        WM_APP_BALLOON => {
            if let Some((title, text)) = lock(&BALLOON).take() {
                balloon(hwnd, &title, &text);
            }
            0
        }
        WM_APP_CLIP_SET => write_clipboard(hwnd, &*(wparam as *const Vec<u16>)) as LRESULT,
        WM_APP_CLIP_GET => {
            *(wparam as *mut Option<String>) = read_clipboard(hwnd);
            0
        }
        m if m != 0 && m == TASKBAR_CREATED.load(Ordering::Relaxed) => {
            // O Explorer reiniciou: o ícone da bandeja precisa ser recriado.
            add_tray(hwnd);
            refresh(hwnd);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

unsafe fn on_create(hwnd: HWND) {
    let instance = GetModuleHandleW(null());
    for (i, spec) in SPECS.iter().enumerate() {
        let ctl = CreateWindowExW(
            0,
            wide(spec.class).as_ptr(),
            wide(spec.text).as_ptr(),
            WS_CHILD | WS_VISIBLE | spec.style,
            0,
            0,
            0,
            0,
            hwnd,
            (ID_BASE + i) as _,
            instance,
            null(),
        );
        UI.with(|ui| ui.controls[i].set(ctl));
    }
    let dpi = GetDpiForWindow(hwnd).max(96);
    apply_dpi(dpi);

    let mut rect = RECT { left: 0, top: 0, right: scale(CLIENT_W, dpi), bottom: scale(CLIENT_H, dpi) };
    AdjustWindowRectExForDpi(&mut rect, WINDOW_STYLE, 0, 0, dpi);
    let (width, height) = (rect.right - rect.left, rect.bottom - rect.top);
    let mut work: RECT = zeroed();
    SystemParametersInfoW(SPI_GETWORKAREA, 0, &mut work as *mut RECT as *mut c_void, 0);
    let x = work.left + ((work.right - work.left - width) / 2).max(0);
    let y = work.top + ((work.bottom - work.top - height) / 2).max(0);
    SetWindowPos(hwnd, null_mut(), x, y, width, height, SWP_NOZORDER | SWP_NOACTIVATE);

    // Uma segunda cópia (sem admin) consegue pedir para esta janela aparecer.
    ChangeWindowMessageFilterEx(hwnd, WM_APP_SHOW, MSGFLT_ALLOW, null_mut());
    SetTimer(hwnd, TIMER_ADDRESSES, 10_000, None);
}

fn make_font(points: i32, weight: u32, dpi: u32) -> HFONT {
    let height = -((points as i64 * dpi as i64 + 36) / 72) as i32;
    unsafe {
        CreateFontW(
            height,
            0,
            0,
            0,
            weight as i32,
            0,
            0,
            0,
            DEFAULT_CHARSET as u32,
            OUT_DEFAULT_PRECIS as u32,
            CLIP_DEFAULT_PRECIS as u32,
            CLEARTYPE_QUALITY as u32,
            (DEFAULT_PITCH | FF_DONTCARE) as u32,
            wide("Segoe UI").as_ptr(),
        )
    }
}

unsafe fn apply_dpi(dpi: u32) {
    let fonts = [make_font(10, FW_NORMAL, dpi), make_font(17, FW_SEMIBOLD, dpi), make_font(11, FW_SEMIBOLD, dpi)];
    let old = UI.with(|ui| {
        ui.dpi.set(dpi);
        [ui.fonts[0].replace(fonts[0]), ui.fonts[1].replace(fonts[1]), ui.fonts[2].replace(fonts[2])]
    });
    for (i, spec) in SPECS.iter().enumerate() {
        let (x, y, w, h) = spec.rect;
        let ctl = control(i);
        SetWindowPos(
            ctl,
            null_mut(),
            scale(x, dpi),
            scale(y, dpi),
            scale(w, dpi),
            scale(h, dpi),
            SWP_NOZORDER | SWP_NOACTIVATE,
        );
        let font = match i {
            C_TITLE => fonts[1],
            C_STATUS => fonts[2],
            _ => fonts[0],
        };
        SendMessageW(ctl, WM_SETFONT, font as WPARAM, 1);
    }
    for font in old {
        if !font.is_null() {
            DeleteObject(font);
        }
    }
}

unsafe fn on_ctlcolor(hdc: HDC, ctl: HWND) -> LRESULT {
    SetBkColor(hdc, GetSysColor(COLOR_WINDOW));
    let color = if ctl == control(C_STATUS) {
        UI.with(|ui| ui.status_color.get())
    } else if ctl == control(C_HELP) {
        HINT
    } else {
        GetSysColor(COLOR_WINDOWTEXT)
    };
    SetTextColor(hdc, color);
    GetSysColorBrush(COLOR_WINDOW) as LRESULT
}

fn checked(index: usize) -> bool {
    unsafe { SendMessageW(control(index), BM_GETCHECK, 0, 0) as u32 == BST_CHECKED }
}

fn set_allow_pairing(allow: bool) {
    if let Some(app) = APP.get() {
        let mut cfg = lock(&app.shared.config);
        cfg.allow_pairing = allow;
        if let Err(e) = cfg.save() {
            crate::log!("não consegui salvar a configuração: {e}");
        }
    }
}

fn allow_pairing() -> bool {
    APP.get().map(|app| lock(&app.shared.config).allow_pairing).unwrap_or(true)
}

unsafe fn on_command(hwnd: HWND, id: usize, code: u32) {
    if code != BN_CLICKED {
        return;
    }
    match id {
        _ if id == ID_BASE + C_REMOVE => remove_selected(hwnd),
        _ if id == ID_BASE + C_AUTOSTART => {
            if !sys::set_autostart(checked(C_AUTOSTART)) {
                sys::message("Não consegui alterar a inicialização automática.", true);
            }
            refresh(hwnd);
        }
        _ if id == ID_BASE + C_ALLOWPAIR => {
            set_allow_pairing(checked(C_ALLOWPAIR));
            refresh(hwnd);
        }
        _ if id == ID_BASE + C_FIREWALL => {
            sys::run_elevated(hwnd, "--configure-firewall");
        }
        _ if id == ID_BASE + C_ADMIN => {
            if sys::run_elevated(hwnd, "--restarted") {
                quit(hwnd);
            }
        }
        _ if id == ID_BASE + C_QUIT || id == ID_MENU_QUIT => quit(hwnd),
        ID_MENU_OPEN => show(hwnd),
        ID_MENU_AUTOSTART => {
            sys::set_autostart(!sys::autostart_enabled());
            refresh(hwnd);
        }
        ID_MENU_ALLOW => {
            set_allow_pairing(!allow_pairing());
            refresh(hwnd);
        }
        IDCANCEL => {
            ShowWindow(hwnd, SW_HIDE);
        }
        _ => {}
    }
}

unsafe fn remove_selected(hwnd: HWND) {
    let Some(app) = APP.get() else { return };
    let index = SendMessageW(control(C_DEVLIST), LB_GETCURSEL, 0, 0);
    let id = UI.with(|ui| usize::try_from(index).ok().and_then(|i| ui.device_ids.borrow().get(i).copied()));
    let Some(id) = id else {
        sys::message("Selecione um celular na lista primeiro.", false);
        return;
    };
    let Some(name) = lock(&app.shared.config).find(&id).map(|d| d.name.clone()) else { return };
    let question =
        wide(&format!("Remover \"{name}\"?\n\nPara usar esse celular de novo será preciso parear outra vez."));
    if MessageBoxW(hwnd, question.as_ptr(), wide("Teclado Remoto").as_ptr(), MB_YESNO | MB_ICONQUESTION) != IDYES {
        return;
    }
    {
        let mut cfg = lock(&app.shared.config);
        cfg.remove(&id);
        if let Err(e) = cfg.save() {
            crate::log!("não consegui salvar a configuração: {e}");
        }
    }
    app.shared.disconnect_device(&id);
    refresh(hwnd);
}

unsafe fn quit(hwnd: HWND) {
    if let Some(app) = APP.get() {
        app.shared.shutdown();
    }
    DestroyWindow(hwnd);
}

unsafe fn show(hwnd: HWND) {
    ShowWindow(hwnd, SW_SHOWNORMAL);
    SetForegroundWindow(hwnd);
}

fn refresh_addresses() {
    let ips: Vec<String> = sys::local_ipv4s().iter().take(3).map(|ip| ip.to_string()).collect();
    let text = if ips.is_empty() { "sem rede".to_string() } else { ips.join(", ") };
    UI.with(|ui| *ui.addresses.borrow_mut() = text);
}

fn format_date(unix: u64) -> String {
    let ticks = unix * 10_000_000 + 116_444_736_000_000_000;
    let utc = FILETIME { dwLowDateTime: ticks as u32, dwHighDateTime: (ticks >> 32) as u32 };
    unsafe {
        let mut local: FILETIME = zeroed();
        let mut time: SYSTEMTIME = zeroed();
        if FileTimeToLocalFileTime(&utc, &mut local) == 0 || FileTimeToSystemTime(&local, &mut time) == 0 {
            return String::new();
        }
        format!("{:02}/{:02}/{}", time.wDay, time.wMonth, time.wYear)
    }
}

unsafe fn set_text(index: usize, text: &str) {
    let ctl = control(index);
    let mut current = [0u16; 512];
    let len = GetWindowTextW(ctl, current.as_mut_ptr(), current.len() as i32).max(0) as usize;
    let new: Vec<u16> = text.encode_utf16().collect();
    if current[..len] != new[..] {
        SetWindowTextW(ctl, wide(text).as_ptr());
    }
}

unsafe fn refresh(hwnd: HWND) {
    let Some(app) = APP.get() else { return };
    let shared = &app.shared;
    let sessions: Vec<([u8; 16], String)> =
        lock(&shared.sessions).iter().map(|s| (s.device_id, format!("{} ({})", s.device_name, s.addr.ip()))).collect();
    let (pc_name, allow, devices) = {
        let cfg = lock(&shared.config);
        (cfg.name.clone(), cfg.allow_pairing, cfg.devices.clone())
    };
    let port = shared.port.load(Ordering::Relaxed);

    let (status, color) = if !app.port_ok {
        (format!("✖ Não consegui abrir a porta de conexão (TCP {port})."), RED)
    } else if sessions.is_empty() {
        ("○ Aguardando o celular…".to_string(), GRAY)
    } else {
        let names: Vec<&str> = sessions.iter().map(|(_, n)| n.as_str()).collect();
        (format!("● Conectado: {}", names.join(", ")), GREEN)
    };
    UI.with(|ui| ui.status_color.set(color));
    set_text(C_STATUS, &status);
    InvalidateRect(control(C_STATUS), null(), 1);
    set_text(C_PC, &format!("Nome deste PC: {pc_name}{}", if app.elevated { "   ·   administrador" } else { "" }));
    let addresses = UI.with(|ui| ui.addresses.borrow().clone());
    set_text(C_ADDR, &format!("Endereço: {addresses}   ·   porta {port}"));

    let list = control(C_DEVLIST);
    let selected = SendMessageW(list, LB_GETCURSEL, 0, 0);
    let selected_id = UI.with(|ui| usize::try_from(selected).ok().and_then(|i| ui.device_ids.borrow().get(i).copied()));
    SendMessageW(list, LB_RESETCONTENT, 0, 0);
    for d in &devices {
        let online = sessions.iter().any(|(id, _)| *id == d.id);
        let line = format!(
            "{}   —   pareado em {}{}",
            d.name,
            format_date(d.paired_at),
            if online { "   ·   conectado" } else { "" }
        );
        SendMessageW(list, LB_ADDSTRING, 0, wide(&line).as_ptr() as LPARAM);
    }
    if let Some(pos) = selected_id.and_then(|id| devices.iter().position(|d| d.id == id)) {
        SendMessageW(list, LB_SETCURSEL, pos, 0);
    }
    UI.with(|ui| *ui.device_ids.borrow_mut() = devices.iter().map(|d| d.id).collect());

    let check = |index: usize, on: bool| {
        SendMessageW(control(index), BM_SETCHECK, if on { BST_CHECKED as WPARAM } else { 0 }, 0);
    };
    check(C_AUTOSTART, sys::autostart_enabled());
    check(C_ALLOWPAIR, allow);
    if app.elevated {
        set_text(C_ADMIN, "Já é administrador");
        EnableWindow(control(C_ADMIN), 0);
    }

    let tip = if sessions.is_empty() {
        "Teclado Remoto — aguardando o celular".to_string()
    } else {
        format!("Teclado Remoto — conectado: {}", sessions[0].1)
    };
    let mut nid = tray_data(hwnd);
    nid.uFlags = NIF_ICON | NIF_TIP;
    nid.hIcon = UI.with(|ui| ui.icons[if sessions.is_empty() { 1 } else { 0 }].get());
    copy_wide(&mut nid.szTip, &tip);
    Shell_NotifyIconW(NIM_MODIFY, &nid);
}

fn copy_wide(target: &mut [u16], text: &str) {
    let units: Vec<u16> = text.encode_utf16().take(target.len() - 1).collect();
    target[..units.len()].copy_from_slice(&units);
    target[units.len()] = 0;
}

fn tray_data(hwnd: HWND) -> NOTIFYICONDATAW {
    let mut nid: NOTIFYICONDATAW = unsafe { zeroed() };
    nid.cbSize = size_of::<NOTIFYICONDATAW>() as u32;
    nid.hWnd = hwnd;
    nid.uID = 1;
    nid
}

unsafe fn add_tray(hwnd: HWND) {
    let mut nid = tray_data(hwnd);
    nid.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
    nid.uCallbackMessage = WM_APP_TRAY;
    nid.hIcon = UI.with(|ui| ui.icons[1].get());
    copy_wide(&mut nid.szTip, "Teclado Remoto");
    Shell_NotifyIconW(NIM_ADD, &nid);
}

unsafe fn balloon(hwnd: HWND, title: &str, text: &str) {
    let mut nid = tray_data(hwnd);
    nid.uFlags = NIF_INFO;
    nid.dwInfoFlags = NIIF_INFO;
    copy_wide(&mut nid.szInfoTitle, title);
    copy_wide(&mut nid.szInfo, text);
    Shell_NotifyIconW(NIM_MODIFY, &nid);
}

unsafe fn tray_menu(hwnd: HWND) {
    let menu = CreatePopupMenu();
    if menu.is_null() {
        return;
    }
    let flag = |on: bool| if on { MF_CHECKED } else { 0 };
    AppendMenuW(menu, MF_STRING, ID_MENU_OPEN, wide("Abrir Teclado Remoto").as_ptr());
    SetMenuDefaultItem(menu, ID_MENU_OPEN as u32, 0);
    AppendMenuW(menu, MF_SEPARATOR, 0, null());
    AppendMenuW(
        menu,
        MF_STRING | flag(sys::autostart_enabled()),
        ID_MENU_AUTOSTART,
        wide("Iniciar junto com o Windows").as_ptr(),
    );
    AppendMenuW(
        menu,
        MF_STRING | flag(allow_pairing()),
        ID_MENU_ALLOW,
        wide("Permitir parear novos celulares").as_ptr(),
    );
    AppendMenuW(menu, MF_SEPARATOR, 0, null());
    AppendMenuW(menu, MF_STRING, ID_MENU_QUIT, wide("Encerrar").as_ptr());
    let mut pt = POINT { x: 0, y: 0 };
    GetCursorPos(&mut pt);
    // Sem isso o menu não fecha ao clicar fora dele.
    SetForegroundWindow(hwnd);
    let command = TrackPopupMenu(menu, TPM_RIGHTBUTTON | TPM_RETURNCMD | TPM_NONOTIFY, pt.x, pt.y, 0, hwnd, null());
    PostMessageW(hwnd, WM_NULL, 0, 0);
    DestroyMenu(menu);
    if command > 0 {
        on_command(hwnd, command as usize, BN_CLICKED);
    }
}
