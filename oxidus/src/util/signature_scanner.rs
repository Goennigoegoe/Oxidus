use std::ffi::{CStr, CString};

use libc::dl_phdr_info;

#[derive(Debug, Clone)]
pub struct Signature {
    pattern: Vec<u8>,
    mask: Vec<u8>,
}

impl Signature {
    pub fn new(pattern: Vec<u8>, mask: Vec<u8>) -> Self {
        assert_eq!(
            pattern.len(),
            mask.len(),
            "Pattern and mask must be the same length"
        );
        Self { pattern, mask }
    }

    pub fn scan(&self, memory: &[u8]) -> Option<usize> {
        sig_scan(memory, &self.pattern, &self.mask)
    }

    pub fn scan_module(&self, module_name: &str) -> Option<*const u8> {
        find_module(module_name).and_then(|module| {
            let memory = unsafe { std::slice::from_raw_parts(module.base, module.size) };
            let raw_res = self.scan(memory);

            raw_res.map(|offset| unsafe { module.base.add(offset) })
        })
    }
}

#[derive(Debug)]
pub struct ModuleInfo {
    pub base: *const u8,
    pub size: usize,
    pub name: String,
}

struct SearchData {
    cname: CString,
    result: Option<ModuleInfo>,
}

unsafe extern "C" fn callback(
    info: *mut dl_phdr_info,
    _size: usize,
    data: *mut libc::c_void,
) -> i32 {
    let info = &*info;
    let search_data = &mut *data.cast::<SearchData>();

    let module_name = if info.dlpi_name.is_null() {
        ""
    } else {
        CStr::from_ptr(info.dlpi_name).to_str().unwrap_or("")
    };

    if module_name.contains(search_data.cname.to_str().unwrap_or("")) {
        let mut max_addr = 0;
        for i in 0..info.dlpi_phnum {
            let phdr = info.dlpi_phdr.add(i as usize).read();
            let end = phdr.p_vaddr + phdr.p_memsz;
            if end > max_addr {
                max_addr = end;
            }
        }

        search_data.result = Some(ModuleInfo {
            base: info.dlpi_addr as *const u8,
            size: (max_addr) as usize,
            name: module_name.to_string(),
        });
        1
    } else {
        0
    }
}

fn find_module(name: &str) -> Option<ModuleInfo> {
    let mut search_data = SearchData {
        cname: CString::new(name.strip_prefix("./").unwrap()).ok()?,
        result: None,
    };

    unsafe {
        libc::dl_iterate_phdr(Some(callback), (&raw mut search_data).cast());
    }

    search_data.result
}

fn sig_scan(memory: &[u8], pattern: &[u8], mask: &[u8]) -> Option<usize> {
    let pattern_len = pattern.len();
    if memory.len() < pattern_len || pattern_len != mask.len() {
        return None;
    }
    'outer: for i in 0..=memory.len().saturating_sub(pattern_len) {
        for (j, (&pat_byte, &mask_byte)) in pattern.iter().zip(mask).enumerate() {
            if mask_byte == b'x' && memory[i + j] != pat_byte {
                continue 'outer;
            }
        }
        return Some(i);
    }
    None
}
