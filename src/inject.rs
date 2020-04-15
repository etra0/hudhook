use winapi::shared::minwindef::*;
use winapi::um::psapi;
use winapi::um::handleapi::*;
use winapi::um::winnt::*;
use winapi::um::winbase::INFINITE;
use winapi::um::memoryapi;
use winapi::um::libloaderapi::{GetProcAddress, GetModuleHandleA};
use winapi::um::minwinbase::LPSECURITY_ATTRIBUTES;
use winapi::um::processthreadsapi;
use winapi::um::synchapi::WaitForSingleObject;
use winapi::ctypes::*;
use std::mem;
use std::ffi::{CString, CStr};

pub fn find_process(s: &str) -> Option<DWORD> {
  let mut lpid_process = [0 as DWORD; 256];
  let mut cb_needed = 0 as DWORD;
  let mut ret = 0;

  unsafe {
    psapi::EnumProcesses(
      lpid_process.as_mut_ptr(),
      mem::size_of_val(&lpid_process) as DWORD,
      &mut cb_needed as *mut DWORD);
  }

  for i in 0..((cb_needed as f64 / mem::size_of::<DWORD>() as f64) as usize) {
    let pid = lpid_process[i];
    let hproc = unsafe {
      processthreadsapi::OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, 0, pid)
    };
    
    if hproc == 0 as *mut c_void {
      continue;
    }

    let mut hmodule = [0 as HMODULE; 64];
    let mut pmcb_needed = 0 as DWORD;
    let mut modname = [0 as CHAR; 128];
    unsafe {
      psapi::EnumProcessModules(
        hproc,
        hmodule.as_mut_ptr(),
        mem::size_of_val(&hmodule) as DWORD,
        &mut pmcb_needed as *mut DWORD);
      psapi::GetModuleBaseNameA(
        hproc, 
        hmodule[0],
        modname.as_mut_ptr(),
        mem::size_of_val(&modname) as DWORD);
    }
    let mn = unsafe { CStr::from_ptr(modname.as_ptr()) }.to_string_lossy().to_lowercase();
    
    if mn == s.to_lowercase() {

      let mut mi = psapi::MODULEINFO {
        lpBaseOfDll: 0 as LPVOID,
        SizeOfImage: 0 as DWORD,
        EntryPoint: 0 as LPVOID
      };
      unsafe {
        psapi::GetModuleInformation(
          hproc,
          hmodule[0], 
          &mut mi as psapi::LPMODULEINFO,
          mem::size_of_val(&mi) as DWORD);
      }
      ret = pid;
    }

    unsafe { CloseHandle(hproc); }
  }

  match ret {
    0 => None,
    i => Some(i)
  }
}

pub fn inject(pid: DWORD, dll: &str) {
  let pathstr = std::fs::canonicalize(dll).unwrap();
  let mut path = [0 as CHAR; MAX_PATH];
  for (dest, src) in path.iter_mut().zip(CString::new(pathstr.to_str().unwrap()).unwrap().into_bytes().into_iter()) {
    *dest = src as _;
  }

  let hproc = unsafe { processthreadsapi::OpenProcess(PROCESS_ALL_ACCESS, 0, pid) };
  let dllp = unsafe { 
    memoryapi::VirtualAllocEx(
      hproc, 0 as LPVOID, MAX_PATH, MEM_RESERVE | MEM_COMMIT, PAGE_READWRITE)
  };

  unsafe {
    memoryapi::WriteProcessMemory(
      hproc, dllp, std::mem::transmute(&path), MAX_PATH, 0 as *mut usize);
  }

  let thread = unsafe {
    let proc_addr = GetProcAddress(GetModuleHandleA(CString::new("Kernel32").unwrap().as_ptr()), CString::new("LoadLibraryA").unwrap().as_ptr());
    processthreadsapi::CreateRemoteThread(
      hproc, 0 as LPSECURITY_ATTRIBUTES, 0, 
      Some(std::mem::transmute(proc_addr)),
      dllp, 0, 0 as *mut DWORD)
  };
  println!("{:?}", thread);

  unsafe {
    WaitForSingleObject(thread, INFINITE);
    let mut ec = 0 as DWORD;
    processthreadsapi::GetExitCodeThread(thread, &mut ec as *mut DWORD);
    CloseHandle(thread);
    memoryapi::VirtualFreeEx(hproc, dllp, 0, MEM_RELEASE);
    CloseHandle(hproc);
  }
}
