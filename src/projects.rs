//! Saved project library (Pro edition).
//!
//! Persists designs to `localStorage` (available in both the hub-hosted
//! browser context and the desktop webview), keyed
//! `tpt-em-design-projects`. One entry per named project: the tool that
//! created it, a timestamp, and the form field values — so the library
//! holds any mix of trace, antenna, and matching projects ("across
//! designs").
//!
//! JSON is handled with `js_sys::Json` (no serde dependency in the wasm
//! payload); values are all strings, exactly what the form fields hold.

use std::rc::Rc;

use tpt_appfront_core::UITree;
use wasm_bindgen::JsCast;

use crate::app::{label, styled_text, write_field, Msg};

const KEY: &str = "tpt-em-design-projects";

/// Form fields captured per project (class names sans the leading dot, used
/// as JSON keys). Trace fields always; antenna/matching fields are stored
/// as-is when present so mixed projects round-trip.
const PROJECT_FIELDS: &[&str] = &[
    "emd-kind",
    "emd-width",
    "emd-height",
    "emd-spacing",
    "emd-copper",
    "emd-er",
    "emd-tand",
    "emd-freq",
    "emd-ant-type",
    "emd-ant-freq",
    "emd-ant-diam",
    "emd-ant-er",
    "emd-ant-h",
    "emd-ant-feed",
    "emd-match-kind",
    "emd-match-z",
    "emd-match-load",
    "emd-match-freq",
    "emd-match-eps",
];

/// One row of the library list.
pub struct ProjectMeta {
    pub name: String,
    pub tool: String,
    pub saved_at: String,
}

fn storage() -> Option<web_sys::Storage> {
    web_sys::window().and_then(|w| w.local_storage().ok().flatten())
}

fn read_store() -> Vec<js_sys::Object> {
    let Some(storage) = storage() else {
        return Vec::new();
    };
    let Ok(Some(text)) = storage.get_item(KEY) else {
        return Vec::new();
    };
    let Ok(value) = js_sys::JSON::parse(&text) else {
        return Vec::new();
    };
    let array: js_sys::Array = match value.dyn_into() {
        Ok(a) => a,
        Err(_) => return Vec::new(),
    };
    array
        .iter()
        .filter_map(|v| v.dyn_into::<js_sys::Object>().ok())
        .collect()
}

fn write_store(entries: &[js_sys::Object]) {
    let Some(storage) = storage() else {
        return;
    };
    let array = js_sys::Array::new();
    for entry in entries {
        array.push(entry.as_ref());
    }
    if let Ok(text) = js_sys::JSON::stringify(&array.into()) {
        let _ = storage.set_item(KEY, &text.as_string().unwrap_or_default());
    }
}

fn get_str(entry: &js_sys::Object, key: &str) -> String {
    js_sys::Reflect::get(entry, &wasm_bindgen::JsValue::from_str(key))
        .ok()
        .and_then(|v| v.as_string())
        .unwrap_or_default()
}

/// Summaries for the library list, newest first.
pub fn list() -> Vec<ProjectMeta> {
    let mut metas: Vec<ProjectMeta> = read_store()
        .into_iter()
        .map(|entry| ProjectMeta {
            name: get_str(&entry, "name"),
            tool: get_str(&entry, "tool"),
            saved_at: get_str(&entry, "saved_at"),
        })
        .collect();
    metas.reverse();
    metas
}

/// Saves the current form under the name in `.emd-project-name` (an existing
/// name is overwritten — it's the library's only naming rule worth having).
pub fn save_from_form(container: &web_sys::Element) {
    let name = crate::app::read_input(container, ".emd-project-name").trim().to_string();
    if name.is_empty() {
        return;
    }
    let kind = crate::app::read_input(container, ".emd-kind");
    let tool = match kind.as_str() {
        "stripline" => "Stripline",
        _ => "Microstrip",
    };

    let entry = js_sys::Object::new();
    let _ = js_sys::Reflect::set(
        &entry,
        &wasm_bindgen::JsValue::from_str("name"),
        &wasm_bindgen::JsValue::from_str(&name),
    );
    let _ = js_sys::Reflect::set(
        &entry,
        &wasm_bindgen::JsValue::from_str("tool"),
        &wasm_bindgen::JsValue::from_str(tool),
    );
    let saved_at = js_sys::Date::new_0().to_locale_string("en-GB", &js_sys::Object::new());
    let _ = js_sys::Reflect::set(
        &entry,
        &wasm_bindgen::JsValue::from_str("saved_at"),
        &saved_at.into(),
    );
    let fields = js_sys::Object::new();
    for field in PROJECT_FIELDS {
        let value = crate::app::read_input(container, &format!(".{field}"));
        if !value.is_empty() {
            let _ = js_sys::Reflect::set(
                &fields,
                &wasm_bindgen::JsValue::from_str(field),
                &wasm_bindgen::JsValue::from_str(&value),
            );
        }
    }
    let _ = js_sys::Reflect::set(
        &entry,
        &wasm_bindgen::JsValue::from_str("fields"),
        &fields.into(),
    );

    let mut entries: Vec<js_sys::Object> = read_store()
        .into_iter()
        .filter(|e| get_str(e, "name") != name)
        .collect();
    entries.push(entry);
    write_store(&entries);
}

/// Writes a saved project's fields back into the form.
pub fn load_into_form(container: &web_sys::Element, name: &str) -> bool {
    let Some(entry) = read_store().into_iter().find(|e| get_str(e, "name") == name) else {
        return false;
    };
    let Ok(fields_value) = js_sys::Reflect::get(&entry, &wasm_bindgen::JsValue::from_str("fields")) else {
        return false;
    };
    let Ok(fields) = fields_value.dyn_into::<js_sys::Object>() else {
        return false;
    };
    for field in PROJECT_FIELDS {
        let value = js_sys::Reflect::get(&fields, &wasm_bindgen::JsValue::from_str(field))
            .ok()
            .and_then(|v| v.as_string());
        if let Some(value) = value {
            write_field(container, &format!(".{field}"), &value);
        }
    }
    let kind = crate::app::read_input(container, ".emd-kind");
    crate::app::toggle_kind_fields(container, &kind);
    true
}

/// Removes a project by name.
pub fn delete(name: &str) {
    let entries: Vec<js_sys::Object> = read_store()
        .into_iter()
        .filter(|e| get_str(e, "name") != name)
        .collect();
    write_store(&entries);
}

/// Sidebar card: name input, save button, and the reactive list placeholder.
pub fn sidebar_card(sidebar: &mut tpt_appfront_core::ContainerBuilder<Msg>) {
    sidebar.container(|card| {
        card.heading(3, "Project library (Pro)");
        card.container(|row| {
            row.container(|field| {
                label(field, "Project name");
                field
                    .input("My design")
                    .class("emd-input emd-project-name");
            })
            .class("emd-field");
        })
        .class("emd-row");
        card.container(|actions| {
            actions
                .button("Save current setup")
                .class("emd-btn")
                .on_click(Msg::SaveProject);
        })
        .class("emd-actions");
        styled_text(
            card,
            "Saved setups keep every field \u{2014} trace, antenna, and matching \
             \u{2014} and survive refreshes and restarts.",
            "emd-hint",
        );
        card.container(|_| {}).class("emd-project-list");
    })
    .class("emd-card");
}

/// The reactive library list.
pub fn list_view(projects: Rc<Vec<ProjectMeta>>) -> UITree<Msg> {
    let mut builder = tpt_appfront_core::ContainerBuilder::<Msg>::new();
    builder.container(|c| {
        if projects.is_empty() {
            styled_text(c, "No saved projects yet \u{2014} name this setup and press Save.", "emd-hint");
        }
        for project in projects.iter() {
            c.container(|row| {
                row.container(|info| {
                    info.container(|r| {
                        r.text(project.name.clone());
                        r.text(format!(" \u{b7} {}", project.tool));
                        styled_text(r, format!(" \u{b7} {}", project.saved_at), "emd-hint");
                    })
                    .class("emd-project-row-name");
                })
                .class("emd-project-info");
                row.container(|actions| {
                    actions
                        .button("Load")
                        .class("emd-btn")
                        .on_click(Msg::LoadProject(project.name.clone()));
                    actions
                        .button("Delete")
                        .class("emd-btn")
                        .on_click(Msg::DeleteProject(project.name.clone()));
                })
                .class("emd-actions-secondary emd-project-actions");
            })
            .class("emd-project-row");
        }
    });
    builder
        .into_only_child()
        .expect("project list view builds exactly one root container")
}
