use crate::platform::ToolResult;
use crate::safety;
use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_ALL};
use windows::Win32::System::Variant::VARIANT;
use windows::Win32::UI::Accessibility::{
    CUIAutomation, IUIAutomation, IUIAutomationCondition, IUIAutomationElement,
    IUIAutomationInvokePattern, TreeScope_Descendants, UIA_ButtonControlTypeId,
    UIA_CheckBoxControlTypeId, UIA_ComboBoxControlTypeId, UIA_ControlTypePropertyId,
    UIA_CustomControlTypeId, UIA_DataItemControlTypeId, UIA_DocumentControlTypeId,
    UIA_EditControlTypeId, UIA_GroupControlTypeId, UIA_HyperlinkControlTypeId,
    UIA_ImageControlTypeId, UIA_InvokePatternId, UIA_ListControlTypeId, UIA_ListItemControlTypeId,
    UIA_MenuControlTypeId, UIA_MenuItemControlTypeId, UIA_NamePropertyId, UIA_PaneControlTypeId,
    UIA_ProgressBarControlTypeId, UIA_RadioButtonControlTypeId, UIA_ScrollBarControlTypeId,
    UIA_SliderControlTypeId, UIA_SpinnerControlTypeId, UIA_TabItemControlTypeId,
    UIA_TableControlTypeId, UIA_TextControlTypeId, UIA_TreeControlTypeId,
    UIA_TreeItemControlTypeId, UIA_WindowControlTypeId, UIA_CONTROLTYPE_ID,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    mouse_event, MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP,
};
use windows::Win32::UI::WindowsAndMessaging::SetCursorPos;

pub fn find_element(
    name: Option<&str>,
    role: Option<&str>,
    window_handle: Option<u64>,
    max_results: usize,
) -> ToolResult {
    safety::delay()?;
    if name.is_none() && role.is_none() && window_handle.is_none() {
        return Err(
            "Provide at least one of name, role, or window_handle to search for UI elements."
                .to_string(),
        );
    }
    let limit = max_results.clamp(1, 100);
    let elements = find_raw(name, role, window_handle, limit)?;
    let items: Vec<String> = elements
        .iter()
        .filter_map(|element| unsafe { describe(element) }.ok())
        .map(|element| {
            format!(
                "{{\"name\":{}, \"role\":{}, \"automation_id\":{}, \"left\":{}, \"top\":{}, \"width\":{}, \"height\":{}, \"enabled\":{}, \"offscreen\":{}}}",
                serde_json::to_string(&element.name).unwrap_or_default(),
                serde_json::to_string(&element.role).unwrap_or_default(),
                serde_json::to_string(&element.automation_id).unwrap_or_default(),
                element.left,
                element.top,
                element.width,
                element.height,
                element.enabled,
                element.offscreen
            )
        })
        .collect();
    safety::log(
        "find_element",
        &format!("name={name:?}, role={role:?}, matches={}", items.len()),
    );
    Ok(format!(
        "{{\"count\":{}, \"elements\":[{}]}}",
        items.len(),
        items.join(", ")
    ))
}

pub fn click_element(name: Option<&str>, role: Option<&str>, index: usize) -> ToolResult {
    safety::delay()?;
    if name.is_none() && role.is_none() {
        return Err("Provide name and/or role to identify the element to click.".to_string());
    }
    let elements = find_raw(name, role, None, 50)?;
    let Some(element) = elements.get(index) else {
        return Err(format!(
            "Element index {index} is out of range; {} match(es) found for name={name:?}, role={role:?}. Call find_element to see matches.",
            elements.len()
        ));
    };
    let label = unsafe { describe(element) }
        .map(|info| format!("'{}' (role '{}')", info.name, info.role))
        .unwrap_or_else(|_| format!("at index {index}"));
    let method = unsafe { invoke_or_click(element)? };
    safety::log(
        "click_element",
        &format!("element={label}, index={index}, method={method}"),
    );
    Ok(format!("Clicked {label} using {method}."))
}

#[derive(Clone, Debug)]
struct ElementInfo {
    name: String,
    role: String,
    automation_id: String,
    left: i32,
    top: i32,
    width: i32,
    height: i32,
    enabled: bool,
    offscreen: bool,
}

fn find_raw(
    name: Option<&str>,
    role: Option<&str>,
    window_handle: Option<u64>,
    limit: usize,
) -> Result<Vec<IUIAutomationElement>, String> {
    unsafe {
        let automation: IUIAutomation = CoCreateInstance(&CUIAutomation, None, CLSCTX_ALL)
            .map_err(|error| format!("Could not start Windows UI Automation: {error}"))?;
        let root = match window_handle {
            Some(handle) => {
                let handle = HWND(handle as usize as *mut core::ffi::c_void);
                automation.ElementFromHandle(handle).map_err(|error| {
                    format!("Window 0x{:X} not found by UI Automation: {error}. Call list_windows for valid handles.", handle.0 as u64)
                })?
            }
            None => automation
                .GetRootElement()
                .map_err(|error| format!("Could not access the desktop UI tree: {error}"))?,
        };

        let mut conditions: Vec<IUIAutomationCondition> = Vec::new();
        if let Some(name) = name {
            let value = VARIANT::from(name);
            conditions.push(
                automation
                    .CreatePropertyCondition(UIA_NamePropertyId, &value)
                    .map_err(|error| format!("Could not build name condition: {error}"))?,
            );
        }
        if let Some(role) = role {
            let control_type = control_type(role)?;
            let value = VARIANT::from(control_type.0);
            conditions.push(
                automation
                    .CreatePropertyCondition(UIA_ControlTypePropertyId, &value)
                    .map_err(|error| format!("Could not build role condition: {error}"))?,
            );
        }
        let condition = match conditions.len() {
            0 => automation
                .CreateTrueCondition()
                .map_err(|error| format!("Could not build search condition: {error}"))?,
            1 => conditions.remove(0),
            _ => {
                let mut combined = conditions.remove(0);
                for next in conditions {
                    combined = automation
                        .CreateAndCondition(&combined, &next)
                        .map_err(|error| format!("Could not combine search conditions: {error}"))?;
                }
                combined
            }
        };

        let collection = root
            .FindAll(TreeScope_Descendants, &condition)
            .map_err(|error| format!("UI Automation search failed: {error}"))?;
        let length = collection
            .Length()
            .map_err(|error| format!("Could not read search results: {error}"))?;
        let mut result = Vec::new();
        for index in 0..length {
            if result.len() >= limit {
                break;
            }
            if let Ok(element) = collection.GetElement(index) {
                result.push(element);
            }
        }
        Ok(result)
    }
}

unsafe fn describe(element: &IUIAutomationElement) -> Result<ElementInfo, String> {
    let rectangle: RECT = element
        .CurrentBoundingRectangle()
        .map_err(|error| format!("Could not read element bounds: {error}"))?;
    Ok(ElementInfo {
        name: element
            .CurrentName()
            .map(|value| value.to_string())
            .unwrap_or_default(),
        role: role_name(
            element
                .CurrentControlType()
                .map(|value| value.0)
                .unwrap_or(0),
        ),
        automation_id: element
            .CurrentAutomationId()
            .map(|value| value.to_string())
            .unwrap_or_default(),
        left: rectangle.left,
        top: rectangle.top,
        width: rectangle.right - rectangle.left,
        height: rectangle.bottom - rectangle.top,
        enabled: element
            .CurrentIsEnabled()
            .map(|value| value.as_bool())
            .unwrap_or(false),
        offscreen: element
            .CurrentIsOffscreen()
            .map(|value| value.as_bool())
            .unwrap_or(false),
    })
}

unsafe fn invoke_or_click(element: &IUIAutomationElement) -> Result<&'static str, String> {
    if let Ok(pattern) =
        element.GetCurrentPatternAs::<IUIAutomationInvokePattern>(UIA_InvokePatternId)
    {
        if pattern.Invoke().is_ok() {
            return Ok("UI Automation InvokePattern");
        }
    }
    let rectangle: RECT = element
        .CurrentBoundingRectangle()
        .map_err(|error| format!("Could not read element bounds: {error}"))?;
    let width = rectangle.right - rectangle.left;
    let height = rectangle.bottom - rectangle.top;
    if width <= 0 || height <= 0 {
        return Err("Element has no on-screen bounds and cannot be clicked.".to_string());
    }
    let x = rectangle.left + width / 2;
    let y = rectangle.top + height / 2;
    SetCursorPos(x, y)
        .map_err(|error| format!("Could not move cursor over the element: {error}"))?;
    mouse_event(MOUSEEVENTF_LEFTDOWN, 0, 0, 0, 0);
    mouse_event(MOUSEEVENTF_LEFTUP, 0, 0, 0, 0);
    Ok("cursor click at element center")
}

fn control_type(role: &str) -> Result<UIA_CONTROLTYPE_ID, String> {
    let control = match role.trim().to_ascii_lowercase().replace([' ', '_', '-'], "").as_str() {
        "button" => UIA_ButtonControlTypeId,
        "edit" | "textbox" | "input" => UIA_EditControlTypeId,
        "text" | "label" => UIA_TextControlTypeId,
        "window" => UIA_WindowControlTypeId,
        "checkbox" => UIA_CheckBoxControlTypeId,
        "combobox" | "dropdown" => UIA_ComboBoxControlTypeId,
        "listitem" => UIA_ListItemControlTypeId,
        "menuitem" => UIA_MenuItemControlTypeId,
        "tabitem" | "tab" => UIA_TabItemControlTypeId,
        "hyperlink" | "link" => UIA_HyperlinkControlTypeId,
        "radiobutton" | "radio" => UIA_RadioButtonControlTypeId,
        "scrollbar" => UIA_ScrollBarControlTypeId,
        "slider" => UIA_SliderControlTypeId,
        "spinner" => UIA_SpinnerControlTypeId,
        "pane" => UIA_PaneControlTypeId,
        "document" => UIA_DocumentControlTypeId,
        "group" => UIA_GroupControlTypeId,
        "image" | "picture" => UIA_ImageControlTypeId,
        "progressbar" => UIA_ProgressBarControlTypeId,
        "table" => UIA_TableControlTypeId,
        "list" => UIA_ListControlTypeId,
        "tree" => UIA_TreeControlTypeId,
        "treeitem" => UIA_TreeItemControlTypeId,
        "menu" => UIA_MenuControlTypeId,
        "dataitem" => UIA_DataItemControlTypeId,
        "custom" => UIA_CustomControlTypeId,
        _ => return Err(format!(
            "Unknown role '{role}'. Use one of: button, edit, text, window, checkbox, combobox, listitem, menuitem, tabitem, hyperlink, radiobutton, slider, pane, document, group, image, progressbar, table, list, tree, menu, custom."
        )),
    };
    Ok(control)
}

fn role_name(control_type: i32) -> String {
    let known = [
        ("button", UIA_ButtonControlTypeId.0),
        ("edit", UIA_EditControlTypeId.0),
        ("text", UIA_TextControlTypeId.0),
        ("window", UIA_WindowControlTypeId.0),
        ("checkbox", UIA_CheckBoxControlTypeId.0),
        ("combobox", UIA_ComboBoxControlTypeId.0),
        ("listitem", UIA_ListItemControlTypeId.0),
        ("menuitem", UIA_MenuItemControlTypeId.0),
        ("tabitem", UIA_TabItemControlTypeId.0),
        ("hyperlink", UIA_HyperlinkControlTypeId.0),
        ("radiobutton", UIA_RadioButtonControlTypeId.0),
        ("scrollbar", UIA_ScrollBarControlTypeId.0),
        ("slider", UIA_SliderControlTypeId.0),
        ("spinner", UIA_SpinnerControlTypeId.0),
        ("pane", UIA_PaneControlTypeId.0),
        ("document", UIA_DocumentControlTypeId.0),
        ("group", UIA_GroupControlTypeId.0),
        ("image", UIA_ImageControlTypeId.0),
        ("progressbar", UIA_ProgressBarControlTypeId.0),
        ("table", UIA_TableControlTypeId.0),
        ("list", UIA_ListControlTypeId.0),
        ("tree", UIA_TreeControlTypeId.0),
        ("treeitem", UIA_TreeItemControlTypeId.0),
        ("menu", UIA_MenuControlTypeId.0),
        ("dataitem", UIA_DataItemControlTypeId.0),
        ("custom", UIA_CustomControlTypeId.0),
    ];
    known
        .iter()
        .find(|(_, value)| *value == control_type)
        .map(|(name, _)| name.to_string())
        .unwrap_or_else(|| format!("control_type_{control_type}"))
}
