fn main() {
    let source = "../../vendor/mts-esp/libMTSClient.cpp";
    println!("cargo:rerun-if-changed={source}");
    println!("cargo:rerun-if-changed=../../vendor/mts-esp/libMTSClient.h");
    cc::Build::new()
        .cpp(true)
        .file(source)
        .flag_if_supported("-std=c++11")
        .compile("weft_mts_client");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("linux") {
        println!("cargo:rustc-link-lib=dl");
    }
}
