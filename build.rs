#[cfg(windows)]
fn main() {
    let mut res = winres::WindowsResource::new();
    res.set_icon("icons/main.ico");
    res.set_icon_with_id("icons/html.ico", "2");
    res.set_icon_with_id("icons/pdf.ico", "3");
    res.compile().unwrap();
}

#[cfg(not(windows))]
fn main() {}
