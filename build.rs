const COMMANDS: &[&str] = &[
    "register_listener",
    "remove_listener",
    "download_model",
    "load_model",
    "load_cached_model",
    "list_cached_models",
    "remove_cached_model",
    "unload_model",
    "create_conversation",
    "create_conversation_from_history",
    "generate",
    "stop_generation",
    "export_conversation",
    "runtime_info",
];

fn main() {
    println!("cargo:rerun-if-changed=ios/Package.swift");
    println!("cargo:rerun-if-changed=ios/Sources");
    println!("cargo:rerun-if-changed=ios/Vendor/LeapSDKMobile/Package.swift");
    println!("cargo:rerun-if-changed=ios/Vendor/LeapSDKMobile/Sources");
    println!("cargo:rerun-if-changed=android/src/main/java/ExamplePlugin.kt");

    tauri_plugin::Builder::new(COMMANDS)
        .android_path("android")
        .ios_path("ios")
        .build();
}
