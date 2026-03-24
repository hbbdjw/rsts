use libloading::{Library, Symbol};
use std::ffi::c_int;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

type InitializeOls = unsafe extern "system" fn() -> c_int;
type DeinitializeOls = unsafe extern "system" fn();
type GetDllStatus = unsafe extern "system" fn() -> u32;
type Wrmsr = unsafe extern "system" fn(index: u32, eax: u32, edx: u32) -> c_int;
type Rdmsr = unsafe extern "system" fn(index: u32, eax: *mut u32, edx: *mut u32) -> c_int;

pub struct WinRing0 {
    _lib: Library,
    wrmsr: Symbol<'static, Wrmsr>,
    rdmsr: Symbol<'static, Rdmsr>,
    #[allow(dead_code)]
    deinit: Symbol<'static, DeinitializeOls>,
}

impl WinRing0 {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        unsafe {
            let dll_path = resolve_dll_path().ok_or_else(|| {
                "Failed to locate WinRing0 DLL in executable directory or via APP3_WINRING0_DLL".to_string()
            })?;

            let lib = Library::new(&dll_path).map_err(|e| {
                let arch_hint = match pe_machine(&dll_path) {
                    Ok(0x014c) if cfg!(target_pointer_width = "64") => {
                        " (detected x86 DLL, current process is x64)"
                    }
                    Ok(0x8664) if cfg!(target_pointer_width = "32") => {
                        " (detected x64 DLL, current process is x86)"
                    }
                    Ok(_) => "",
                    Err(_) => "",
                };
                format!(
                    "Failed to load WinRing0 DLL from {}: {}{}",
                    dll_path.display(),
                    e,
                    arch_hint
                )
            })?;

            let init: Symbol<InitializeOls> = lib.get(b"InitializeOls")?;
            let deinit: Symbol<DeinitializeOls> = lib.get(b"DeinitializeOls")?;
            let get_status: Symbol<GetDllStatus> = lib.get(b"GetDllStatus")?;
            let wrmsr: Symbol<Wrmsr> = lib.get(b"Wrmsr")?;
            let rdmsr: Symbol<Rdmsr> = lib.get(b"Rdmsr")?;

            let old_cwd = std::env::current_dir().ok();
            if let Some(parent) = dll_path.parent() {
                let _ = std::env::set_current_dir(parent);
            }

            let init_ret = init();
            let status = get_status();

            if let Some(dir) = old_cwd {
                let _ = std::env::set_current_dir(dir);
            }

            if init_ret == 0 {
                if status == 2 {
                    if !is_elevated() {
                        return Err("Failed to initialize WinRing0 driver. DllStatus=2 (run terminal as Administrator)".into());
                    }
                    return Err("Failed to initialize WinRing0 driver. DllStatus=2 (driver service blocked or DLL/SYS pair mismatch)".into());
                }
                return Err(format!("Failed to initialize WinRing0 driver. DllStatus={}", status).into());
            }

            if status != 0 {
                deinit();
                return Err(format!("WinRing0 driver status error: {}", status).into());
            }

            let wrmsr = std::mem::transmute(wrmsr);
            let rdmsr = std::mem::transmute(rdmsr);
            let deinit = std::mem::transmute(deinit);

            Ok(Self {
                _lib: lib,
                wrmsr,
                rdmsr,
                deinit,
            })
        }
    }

    pub fn write_msr(&self, index: u32, value: u64) -> bool {
        let eax = (value & 0xFFFFFFFF) as u32;
        let edx = (value >> 32) as u32;
        unsafe { (self.wrmsr)(index, eax, edx) != 0 }
    }

    pub fn read_msr(&self, index: u32) -> Option<u64> {
        let mut eax: u32 = 0;
        let mut edx: u32 = 0;
        unsafe {
            if (self.rdmsr)(index, &mut eax, &mut edx) != 0 {
                Some((eax as u64) | ((edx as u64) << 32))
            } else {
                None
            }
        }
    }
}

pub fn winring0_search_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    
    // First, try to find it in the same directory as the executable
    if let Ok(mut exe_path) = std::env::current_exe() {
        exe_path.pop(); // Get directory containing the exe
        
        if cfg!(target_pointer_width = "64") {
            paths.push(exe_path.join("WinRing0x64.dll"));
        } else {
            paths.push(exe_path.join("WinRing0.dll"));
        }
    }

    // Fallback to environment variable if set
    if let Ok(p) = std::env::var("APP3_WINRING0_DLL") {
        paths.push(PathBuf::from(p));
    }
    
    paths
}

fn resolve_dll_path() -> Option<PathBuf> {
    winring0_search_paths().into_iter().find(|p| p.exists())
}

fn pe_machine(path: &PathBuf) -> Result<u16, Box<dyn std::error::Error>> {
    let bytes = fs::read(path)?;
    if bytes.len() < 0x40 {
        return Err("invalid pe file".into());
    }
    let pe_offset =
        u32::from_le_bytes([bytes[0x3c], bytes[0x3d], bytes[0x3e], bytes[0x3f]]) as usize;
    if bytes.len() < pe_offset + 6 {
        return Err("invalid pe header".into());
    }
    let machine = u16::from_le_bytes([bytes[pe_offset + 4], bytes[pe_offset + 5]]);
    Ok(machine)
}

fn is_elevated() -> bool {
    Command::new("net")
        .arg("session")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

impl Drop for WinRing0 {
    fn drop(&mut self) {
        // (self.deinit)();
    }
}
