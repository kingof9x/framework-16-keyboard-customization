fn main() {
    // The Windows 11 look, like White Rabbit; colors follow the Windows theme (see ui/theme.slint)
    let config = slint_build::CompilerConfiguration::new().with_style("fluent".into());
    slint_build::compile_with_config("ui/app.slint", config)
        .expect("the window's .slint files have an error");
}
