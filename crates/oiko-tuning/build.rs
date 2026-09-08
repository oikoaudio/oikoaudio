fn main() {
    cc::Build::new()
        .cpp(true)
        .std("c++20")
        .include("../../vendor/tuning-library/include")
        .file("native/tuning.cpp")
        .compile("oiko_tuning");
    println!("cargo:rerun-if-changed=native/tuning.cpp");
    println!("cargo:rerun-if-changed=../../vendor/tuning-library/include");
}
