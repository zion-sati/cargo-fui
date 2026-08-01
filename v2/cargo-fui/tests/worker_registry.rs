use cargo_fui::{generate_native_worker_registry, NativeWorkerRegistryEntry};

fn entry(artifact: &str, entry: &str, native_crate: &str) -> NativeWorkerRegistryEntry {
    NativeWorkerRegistryEntry {
        artifact: artifact.to_owned(),
        entry: entry.to_owned(),
        native_crate: native_crate.to_owned(),
        host_services: vec!["clockNow".to_owned()],
    }
}

#[test]
fn registry_generation_is_deterministic_and_references_every_entry() {
    let alpha = entry("./z-workers.wasm", "alpha", "z_worker");
    let beta = entry("./a-workers.wasm", "beta", "a_worker");
    let zeta = entry("./z-workers.wasm", "zeta", "z_worker");
    let forward = generate_native_worker_registry(&[zeta.clone(), alpha.clone(), beta.clone()]);
    let reversed = generate_native_worker_registry(&[beta, alpha, zeta]);
    assert_eq!(forward, reversed);
    assert_eq!(forward.entry_count, 3);
    assert!(forward.source.contains("invoke: a_worker::beta"));
    assert!(forward.source.contains("invoke: z_worker::alpha"));
    assert!(forward.source.contains("fui_native_worker_registry"));
    assert!(forward.source.contains("HOST_SERVICE_NAME_0_0"));
    assert!(forward.source.contains("host_service_count: 1"));
    assert!(forward.source.find("a_worker::beta") < forward.source.find("z_worker::alpha"));
}

#[test]
fn empty_registry_is_valid_and_contains_no_placeholder_symbol() {
    let registry = generate_native_worker_registry(&[]);
    assert_eq!(registry.entry_count, 0);
    assert!(registry.source.contains("*count = 0"));
    assert!(registry.source.contains("::std::ptr::null()"));
}
