//! Sous Windows : intègre l'icône et les informations de version (éditeur Lopjoy) dans les exécutables.
fn main() {
    println!("cargo:rerun-if-changed=assets/icone.ico");
    #[cfg(windows)]
    {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/icone.ico")
            .set("CompanyName", "Lopjoy")
            .set("ProductName", "AG03 MK2 Studio")
            .set("FileDescription", "AG03 MK2 Studio - contrôle de la Yamaha AG03 MK2")
            .set("LegalCopyright", "Copyright 2026 Lopjoy - Apache 2.0");
        if let Err(e) = res.compile() {
            // Sans le SDK Windows (rc.exe), on compile quand même, simplement sans icône intégrée
            println!("cargo:warning=icône et informations de version non intégrées : {e}");
        }
    }
}
