fn main() {
    let config = slint_build::CompilerConfiguration::new().with_style("material".into());
    slint_build::compile_with_config("ui/app.slint", config).unwrap();

    #[cfg(target_os = "windows")]
    {
        let mut res = winres::WindowsResource::new();
        res.set_icon("resources/icon.ico");
        res.set("FileDescription", "NimbusScroll");
        res.set("ProductName", "NimbusScroll");
        res.set("CompanyName", "qwaq");
        res.set("LegalCopyright", "© qwaq");
        res.set("FileVersion", "1.0.0.0");
        res.set("ProductVersion", "1.0.0.0");
        res.compile().expect("Не удалось скомпилировать ресурс");
    }
}
