//! Chamadas diretas à API do Windows usadas pelo resto do programa.

use std::ffi::c_void;
use std::net::Ipv4Addr;
use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::process::Command;
use std::ptr::{null, null_mut};

use windows_sys::Win32::Foundation::{
    CloseHandle, GetLastError, LocalFree, ERROR_ACCESS_DENIED, ERROR_ALREADY_EXISTS, ERROR_BUFFER_OVERFLOW,
    ERROR_SUCCESS, HANDLE, HWND,
};
use windows_sys::Win32::NetworkManagement::IpHelper::{
    GetAdaptersAddresses, GAA_FLAG_INCLUDE_GATEWAYS, GAA_FLAG_SKIP_ANYCAST, GAA_FLAG_SKIP_DNS_SERVER,
    GAA_FLAG_SKIP_MULTICAST, IF_TYPE_SOFTWARE_LOOPBACK, IP_ADAPTER_ADDRESSES_LH,
};
use windows_sys::Win32::NetworkManagement::Ndis::IfOperStatusUp;
use windows_sys::Win32::Networking::WinSock::{AF_INET, SOCKADDR_IN};
use windows_sys::Win32::Security::Cryptography::{
    CryptProtectData, CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
};
use windows_sys::Win32::Security::{GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY};
use windows_sys::Win32::System::Registry::{
    RegDeleteKeyValueW, RegGetValueW, RegSetKeyValueW, HKEY_CURRENT_USER, REG_SZ, RRF_RT_REG_SZ,
};
use windows_sys::Win32::System::SystemInformation::{ComputerNameDnsHostname, GetComputerNameExW};
use windows_sys::Win32::System::Threading::{
    CreateMutexW, GetCurrentProcess, GetCurrentThread, OpenProcess, OpenProcessToken, SetThreadPriority,
    CREATE_NO_WINDOW, PROCESS_QUERY_LIMITED_INFORMATION, THREAD_PRIORITY_HIGHEST,
};
use windows_sys::Win32::UI::Shell::ShellExecuteW;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    MessageBoxW, MB_ICONINFORMATION, MB_ICONWARNING, MB_OK, MB_TOPMOST, SW_SHOWNORMAL,
};

/// String terminada em zero para a API "W".
pub fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}

pub fn exe_path() -> PathBuf {
    std::env::current_exe().unwrap_or_else(|_| PathBuf::from("TecladoRemoto.exe"))
}

pub fn config_dir() -> PathBuf {
    match std::env::var_os("APPDATA") {
        Some(dir) => PathBuf::from(dir).join("TecladoRemoto"),
        None => exe_path().with_file_name("TecladoRemoto-dados"),
    }
}

pub fn computer_name() -> String {
    let mut buf = [0u16; 256];
    let mut len = buf.len() as u32;
    if unsafe { GetComputerNameExW(ComputerNameDnsHostname, buf.as_mut_ptr(), &mut len) } != 0 && len > 0 {
        return String::from_utf16_lossy(&buf[..len as usize]);
    }
    std::env::var("COMPUTERNAME").unwrap_or_else(|_| "Meu PC".into())
}

pub fn message(text: &str, warning: bool) {
    let icon = if warning { MB_ICONWARNING } else { MB_ICONINFORMATION };
    unsafe {
        MessageBoxW(null_mut(), wide(text).as_ptr(), wide("Teclado Remoto").as_ptr(), MB_OK | MB_TOPMOST | icon);
    }
}

fn blob_call(
    data: &[u8],
    f: unsafe extern "system" fn(*const CRYPT_INTEGER_BLOB, *const CRYPT_INTEGER_BLOB, *mut CRYPT_INTEGER_BLOB) -> i32,
) -> Option<Vec<u8>> {
    let input = CRYPT_INTEGER_BLOB { cbData: data.len() as u32, pbData: data.as_ptr() as *mut u8 };
    let mut output = CRYPT_INTEGER_BLOB { cbData: 0, pbData: null_mut() };
    unsafe {
        if f(&input, null(), &mut output) == 0 || output.pbData.is_null() {
            return None;
        }
        let out = std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec();
        LocalFree(output.pbData as *mut c_void);
        Some(out)
    }
}

unsafe extern "system" fn protect_raw(
    input: *const CRYPT_INTEGER_BLOB,
    entropy: *const CRYPT_INTEGER_BLOB,
    output: *mut CRYPT_INTEGER_BLOB,
) -> i32 {
    CryptProtectData(input, null(), entropy, null(), null(), CRYPTPROTECT_UI_FORBIDDEN, output)
}

unsafe extern "system" fn unprotect_raw(
    input: *const CRYPT_INTEGER_BLOB,
    entropy: *const CRYPT_INTEGER_BLOB,
    output: *mut CRYPT_INTEGER_BLOB,
) -> i32 {
    CryptUnprotectData(input, null_mut(), entropy, null(), null(), CRYPTPROTECT_UI_FORBIDDEN, output)
}

/// Chave de pareamento cifrada pela conta do Windows (DPAPI).
pub fn protect(data: &[u8]) -> Vec<u8> {
    blob_call(data, protect_raw).unwrap_or_default()
}

pub fn unprotect(blob: &[u8]) -> Option<Vec<u8>> {
    blob_call(blob, unprotect_raw)
}

pub fn boost_thread_priority() {
    unsafe {
        SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_HIGHEST);
    }
}

fn token_elevated(process: HANDLE) -> Option<bool> {
    unsafe {
        let mut token: HANDLE = null_mut();
        if OpenProcessToken(process, TOKEN_QUERY, &mut token) == 0 {
            return None;
        }
        let mut elevation = TOKEN_ELEVATION { TokenIsElevated: 0 };
        let mut len = 0u32;
        let ok = GetTokenInformation(
            token,
            TokenElevation,
            &mut elevation as *mut _ as *mut c_void,
            std::mem::size_of::<TOKEN_ELEVATION>() as u32,
            &mut len,
        );
        CloseHandle(token);
        (ok != 0).then_some(elevation.TokenIsElevated != 0)
    }
}

pub fn is_elevated() -> bool {
    token_elevated(unsafe { GetCurrentProcess() }).unwrap_or(false)
}

/// Processo dono da janela está elevado (o Windows descarta nossa digitação nele).
pub fn process_elevated(pid: u32) -> bool {
    unsafe {
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if process.is_null() {
            return false;
        }
        let result = token_elevated(process);
        let denied = GetLastError() == ERROR_ACCESS_DENIED;
        CloseHandle(process);
        result.unwrap_or(denied)
    }
}

/// `false` se outra cópia já está aberta. Com `wait`, espera a cópia antiga fechar
/// (usado ao reiniciar como administrador).
pub fn acquire_single_instance(wait: bool) -> bool {
    let name = wide("Local\\TecladoRemoto.Instancia");
    for _ in 0..if wait { 50 } else { 1 } {
        unsafe {
            let handle = CreateMutexW(null(), 0, name.as_ptr());
            if !handle.is_null() && GetLastError() != ERROR_ALREADY_EXISTS {
                // Fica aberto até o processo terminar.
                return true;
            }
            if !handle.is_null() {
                CloseHandle(handle);
            }
        }
        if wait {
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    }
    false
}

const RUN_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
const RUN_VALUE: &str = "TecladoRemoto";

fn autostart_command() -> String {
    format!("\"{}\" --tray", exe_path().display())
}

fn autostart_value() -> Option<String> {
    let key = wide(RUN_KEY);
    let value = wide(RUN_VALUE);
    let mut buf = [0u16; 1024];
    let mut size = (buf.len() * 2) as u32;
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            key.as_ptr(),
            value.as_ptr(),
            RRF_RT_REG_SZ,
            null_mut(),
            buf.as_mut_ptr() as *mut c_void,
            &mut size,
        )
    };
    if status != ERROR_SUCCESS {
        return None;
    }
    let len = (size as usize / 2).saturating_sub(1).min(buf.len());
    Some(String::from_utf16_lossy(&buf[..len]))
}

pub fn autostart_enabled() -> bool {
    autostart_value().is_some()
}

pub fn set_autostart(enabled: bool) -> bool {
    let key = wide(RUN_KEY);
    let value = wide(RUN_VALUE);
    unsafe {
        if enabled {
            let data = wide(&autostart_command());
            RegSetKeyValueW(
                HKEY_CURRENT_USER,
                key.as_ptr(),
                value.as_ptr(),
                REG_SZ,
                data.as_ptr() as *const c_void,
                (data.len() * 2) as u32,
            ) == ERROR_SUCCESS
        } else {
            RegDeleteKeyValueW(HKEY_CURRENT_USER, key.as_ptr(), value.as_ptr()) == ERROR_SUCCESS
        }
    }
}

/// Se o .exe mudou de pasta, a inicialização automática passa a apontar para o novo lugar.
pub fn refresh_autostart_path() {
    if let Some(current) = autostart_value() {
        if current != autostart_command() {
            set_autostart(true);
        }
    }
}

/// Abre esta mesma aplicação como administrador (pede confirmação do UAC).
pub fn run_elevated(hwnd: HWND, args: &str) -> bool {
    let exe = wide(&exe_path().to_string_lossy());
    let args = wide(args);
    let result =
        unsafe { ShellExecuteW(hwnd, wide("runas").as_ptr(), exe.as_ptr(), args.as_ptr(), null(), SW_SHOWNORMAL) };
    result as usize > 32
}

/// Roda já elevado (via `--configure-firewall`): remove regras antigas deste .exe,
/// inclusive bloqueios criados ao clicar "Cancelar" no aviso do firewall, e libera a entrada.
pub fn configure_firewall() {
    let program = format!("program={}", exe_path().display());
    let netsh = |args: &[&str]| {
        Command::new("netsh")
            .args(args)
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    };
    netsh(&["advfirewall", "firewall", "delete", "rule", "name=all", &program]);
    let added = netsh(&[
        "advfirewall",
        "firewall",
        "add",
        "rule",
        "name=Teclado Remoto",
        "dir=in",
        "action=allow",
        &program,
        "enable=yes",
        "profile=any",
    ]);
    if added {
        message(
            "Pronto! O Teclado Remoto foi liberado no Firewall do Windows.\n\nTente conectar pelo celular de novo.",
            false,
        );
    } else {
        message(
            "Não consegui alterar o Firewall do Windows. Libere o Teclado Remoto manualmente em \
             Segurança do Windows > Firewall e proteção de rede > Permitir um aplicativo pelo firewall.",
            true,
        );
    }
}

/// IPv4 das placas de rede ativas; as que têm gateway (a rede de verdade) vêm primeiro.
pub fn local_ipv4s() -> Vec<Ipv4Addr> {
    let flags = GAA_FLAG_INCLUDE_GATEWAYS | GAA_FLAG_SKIP_ANYCAST | GAA_FLAG_SKIP_MULTICAST | GAA_FLAG_SKIP_DNS_SERVER;
    let mut size: u32 = 16 * 1024;
    let mut buf: Vec<u64> = Vec::new();
    for _ in 0..3 {
        buf = vec![0u64; size as usize / 8 + 1];
        let status = unsafe {
            GetAdaptersAddresses(
                AF_INET as u32,
                flags,
                null(),
                buf.as_mut_ptr() as *mut IP_ADAPTER_ADDRESSES_LH,
                &mut size,
            )
        };
        if status == ERROR_SUCCESS {
            break;
        }
        if status != ERROR_BUFFER_OVERFLOW {
            return Vec::new();
        }
    }
    let mut with_gateway = Vec::new();
    let mut others = Vec::new();
    let mut adapter = buf.as_ptr() as *const IP_ADAPTER_ADDRESSES_LH;
    unsafe {
        while !adapter.is_null() {
            let a = &*adapter;
            if a.OperStatus == IfOperStatusUp && a.IfType != IF_TYPE_SOFTWARE_LOOPBACK {
                let mut unicast = a.FirstUnicastAddress;
                while !unicast.is_null() {
                    let sockaddr = (*unicast).Address.lpSockaddr;
                    if !sockaddr.is_null() && (*sockaddr).sa_family == AF_INET {
                        let sin = &*(sockaddr as *const SOCKADDR_IN);
                        let ip = Ipv4Addr::from(u32::from_be(sin.sin_addr.S_un.S_addr));
                        if !ip.is_link_local() && !ip.is_loopback() {
                            if a.FirstGatewayAddress.is_null() {
                                others.push(ip);
                            } else {
                                with_gateway.push(ip);
                            }
                        }
                    }
                    unicast = (*unicast).Next;
                }
            }
            adapter = a.Next;
        }
    }
    with_gateway.extend(others);
    with_gateway.dedup();
    with_gateway
}
