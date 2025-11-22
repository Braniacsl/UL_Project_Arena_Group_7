// tests/bindings.rs
#[test]
fn generate_bindings() {
    // This forces the lazy_static (if any) or just ensures the types are compiled.
    // Actually, ts-rs generates files during the compilation of the structs themselves
    // when "cargo test" is run.
    // We just need a dummy test to trigger the build profile.
    assert_eq!(1, 1);
}
