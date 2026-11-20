fn main() {
    // The Windows 11 look, like White Rabbit; colors follow the Windows theme (see ui/theme.slint)
    let config = slint_build::CompilerConfiguration::new().with_style("fluent".into());
    slint_build::compile_with_config("ui/app.slint", config)
        .expect("the window's .slint files have an error");
    // The app icon in the .exe. Needs the Windows resource compiler (MSVC builds in CI).
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_env = std::env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    if target_os == "windows" && target_env == "msvc" {
        embed_resource::compile("assets/app.rc", embed_resource::NONE)
            .manifest_optional()
            .expect("could not add the app icon");
    }
}
