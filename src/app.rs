//! Free WASM edition UI (tpt-appfront-dom).
//!
//! Phase-0 placeholder: proves the mount contract end to end. The full
//! calculator UI replaces `mount_app`'s contents in Phase 2 (mount-once
//! shell + reactive results section, mirroring tpt-app-fea-lite).

#[cfg(feature = "pro")]
use wasm_bindgen::prelude::*;

/// Placeholder hello-world shell; replaced by the real UI in Phase 2.
pub fn mount_app(container: &web_sys::Element) -> Result<(), wasm_bindgen::JsValue> {
    let document = web_sys::window()
        .expect("no window")
        .document()
        .expect("no document");
    let heading = document.create_element("h1")?;
    heading.set_text_content(Some("TPT EM Design"));
    container.append_child(&heading)?;
    let para = document.create_element("p")?;
    para.set_text_content(Some("Scaffold proof — trace calculator lands in Phase 2."));
    container.append_child(&para)?;
    Ok(())
}
