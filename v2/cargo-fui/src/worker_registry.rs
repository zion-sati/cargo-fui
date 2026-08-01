#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeWorkerRegistryEntry {
    pub artifact: String,
    pub entry: String,
    pub native_crate: String,
    pub host_services: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeWorkerRegistrySource {
    pub source: String,
    pub entry_count: usize,
}

pub fn generate_native_worker_registry(
    entries: &[NativeWorkerRegistryEntry],
) -> NativeWorkerRegistrySource {
    let mut entries = entries.to_vec();
    entries.sort_by(|left, right| {
        (&left.artifact, &left.entry, &left.native_crate).cmp(&(
            &right.artifact,
            &right.entry,
            &right.native_crate,
        ))
    });

    let mut source = String::from(
        "#[repr(C)]\n\
pub struct FuiNativeWorkerRegistryEntry {\n\
    pub artifact: *const u8,\n\
    pub entry: *const u8,\n\
    pub host_services: *const FuiNativeWorkerHostServiceEntry,\n\
    pub host_service_count: usize,\n\
    pub invoke: unsafe extern \"C\" fn(usize, u32),\n\
}\n\
unsafe impl Sync for FuiNativeWorkerRegistryEntry {}\n\n\
#[repr(C)]\n\
pub struct FuiNativeWorkerHostServiceEntry {\n\
    pub name: *const u8,\n\
}\n\
unsafe impl Sync for FuiNativeWorkerHostServiceEntry {}\n\n",
    );
    for (index, item) in entries.iter().enumerate() {
        source.push_str(&format!(
            "static ARTIFACT_{index}: &[u8] = b\"{}\\0\";\n\
static ENTRY_{index}: &[u8] = b\"{}\\0\";\n",
            rust_byte_string(&item.artifact),
            rust_byte_string(&item.entry),
        ));
        for (service_index, service) in item.host_services.iter().enumerate() {
            source.push_str(&format!(
                "static HOST_SERVICE_NAME_{index}_{service_index}: &[u8] = b\"{}\\0\";\n",
                rust_byte_string(service),
            ));
        }
        if !item.host_services.is_empty() {
            source.push_str(&format!(
                "static HOST_SERVICES_{index}: &[FuiNativeWorkerHostServiceEntry] = &[\n"
            ));
            for service_index in 0..item.host_services.len() {
                source.push_str(&format!(
                    "    FuiNativeWorkerHostServiceEntry {{ name: HOST_SERVICE_NAME_{index}_{service_index}.as_ptr() }},\n"
                ));
            }
            source.push_str("];\n");
        }
    }
    if entries.is_empty() {
        source.push_str(
            "#[no_mangle]\n\
pub unsafe extern \"C\" fn fui_native_worker_registry(count: *mut usize) -> *const FuiNativeWorkerRegistryEntry {\n\
    if !count.is_null() { unsafe { *count = 0; } }\n\
    ::std::ptr::null()\n\
}\n",
        );
    } else {
        source.push_str("\nstatic WORKERS: &[FuiNativeWorkerRegistryEntry] = &[\n");
        for (index, item) in entries.iter().enumerate() {
            source.push_str(&format!(
                "    FuiNativeWorkerRegistryEntry {{ artifact: ARTIFACT_{index}.as_ptr(), entry: ENTRY_{index}.as_ptr(), host_services: {}, host_service_count: {}, invoke: {}::{} }},\n",
                if item.host_services.is_empty() { "::std::ptr::null()".to_owned() } else { format!("HOST_SERVICES_{index}.as_ptr()") },
                item.host_services.len(), item.native_crate, item.entry
            ));
        }
        source.push_str(
            "];\n\n#[no_mangle]\n\
pub unsafe extern \"C\" fn fui_native_worker_registry(count: *mut usize) -> *const FuiNativeWorkerRegistryEntry {\n\
    if !count.is_null() { unsafe { *count = WORKERS.len(); } }\n\
    WORKERS.as_ptr()\n\
}\n",
        );
    }
    NativeWorkerRegistrySource {
        source,
        entry_count: entries.len(),
    }
}

fn rust_byte_string(value: &str) -> String {
    value
        .bytes()
        .map(|byte| match byte {
            b'\\' => "\\\\".to_owned(),
            b'"' => "\\\"".to_owned(),
            0x20..=0x7e => char::from(byte).to_string(),
            _ => format!("\\x{byte:02x}"),
        })
        .collect()
}
