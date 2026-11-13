// Embeds the icon and version info into the Windows .exe. Does nothing on other systems.
fn main() {
    println!("cargo:rerun-if-changed=assets/icon.ico");
    #[cfg(windows)]
    {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/icon.ico");
        res.set("ProductName", "Nazo Makaimura English Patcher");
        res.set("FileDescription", "Nazo Makaimura English Patcher");
        res.set("CompanyName", "Fan translation (unofficial)");
        res.set(
            "LegalCopyright",
            "Patcher code: MIT License. Not affiliated with Capcom.",
        );
        if let Err(e) = res.compile() {
            // Don't fail the whole build over the icon.
            println!("cargo:warning=could not embed icon: {e}");
        }
    }
}
