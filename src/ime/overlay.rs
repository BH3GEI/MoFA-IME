const OVERLAY_WIDTH: f64 = 560.0;
const OVERLAY_HEIGHT: f64 = 50.0;
const OVERLAY_BOTTOM_MARGIN: f64 = 24.0;
const OVERLAY_TOP_MARGIN: f64 = 24.0;
const OVERLAY_SWITCH_DISTANCE: f64 = 210.0;
const OVERLAY_STATUS_BADGE_X: f64 = 16.0;
const OVERLAY_STATUS_BADGE_WIDTH: f64 = 92.0;
const OVERLAY_STATUS_BADGE_HEIGHT: f64 = 33.0;
const OVERLAY_STATUS_TEXT_X_OFFSET: f64 = -OVERLAY_WIDTH + 85.0;
const OVERLAY_STATUS_TEXT_Y_OFFSET: f64 = -(OVERLAY_HEIGHT * 0.5) + 17.0;
const OVERLAY_PREVIEW_MAX_LINES: usize = 6;
const OVERLAY_PREVIEW_LINE_HEIGHT: f64 = 17.0;
const OVERLAY_PREVIEW_MIN_HEIGHT: f64 = 20.0;
const OVERLAY_PREVIEW_LINE_CAP: f32 = 24.0;
const OVERLAY_MAX_HEIGHT: f64 = 158.0;
const ASR_PREVIEW_HOLD_MS: u64 = 900;
const RESULT_OVERLAY_HOLD_MS: u64 = 950;
const OVERLAY_FADE_TOTAL_MS: u64 = 120;
const OVERLAY_FADE_STEPS: u64 = 4;
const SILENCE_RMS_THRESHOLD: f32 = 0.0015;

// History window constants
const HISTORY_WIDTH: f64 = 280.0;
const HISTORY_HEIGHT: f64 = 120.0;
const HISTORY_MARGIN: f64 = 24.0;
const HISTORY_ITEM_HEIGHT: f64 = 32.0;

// Floating orb constants
const ORB_SIZE: f64 = 48.0;
const ORB_MARGIN: f64 = 16.0;

// Global state for orb click handling
static mut ORB_CLICK_TX: Option<std::sync::mpsc::Sender<OrbCommand>> = None;
static mut ORB_WINDOW_PTR: usize = 0;

// History storage (max 50 items)
const MAX_HISTORY_ITEMS: usize = 50;

fn history_items() -> &'static Mutex<Vec<String>> {
    static HISTORY: OnceLock<Mutex<Vec<String>>> = OnceLock::new();
    HISTORY.get_or_init(|| Mutex::new(Vec::new()))
}

pub fn add_history_item(text: &str, overlay: OverlayHandle) {
    if text.trim().is_empty() {
        return;
    }
    let mut items = history_items().lock().unwrap();
    items.insert(0, text.to_string());
    if items.len() > MAX_HISTORY_ITEMS {
        items.pop();
    }
    // Refresh history window if it's visible
    drop(items); // Release lock before calling refresh
    overlay.refresh_history_if_visible();
}

pub fn get_history_items() -> Vec<String> {
    history_items().lock().unwrap().clone()
}

pub fn clear_history() {
    history_items().lock().unwrap().clear();
}

#[derive(Clone, Copy, Debug)]
pub enum OrbCommand {
    ToggleHistory,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct AxPoint {
    x: f64,
    y: f64,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct AxSize {
    width: f64,
    height: f64,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct AxRect {
    origin: AxPoint,
    size: AxSize,
}

type AXValueRef = *const c_void;
type AXValueType = u32;
const K_AX_VALUE_CGRECT_TYPE: AXValueType = 3;

// CALayer constants
const K_CALAYER_GRAVITY_CENTER: &str = "center";

// Orb click handler class
static mut ORB_DELEGATE_CLASS: *const objc::runtime::Class = std::ptr::null();

fn get_orb_delegate_class() -> &'static objc::runtime::Class {
    unsafe {
        if !ORB_DELEGATE_CLASS.is_null() {
            return &*ORB_DELEGATE_CLASS;
        }

        let superclass = objc::runtime::Class::get("NSObject").unwrap();
        let mut decl = objc::declare::ClassDecl::new("OrbClickDelegate", superclass).unwrap();

        decl.add_ivar::<usize>("click_tx_ptr");

        extern "C" fn on_click(this: &objc::runtime::Object, _sel: objc::runtime::Sel, _sender: id) {
            unsafe {
                let tx_ptr: usize = *this.get_ivar("click_tx_ptr");
                if tx_ptr != 0 {
                    let tx = &*(tx_ptr as *const std::sync::mpsc::Sender<OrbCommand>);
                    let _ = tx.send(OrbCommand::ToggleHistory);
                }
            }
        }

        decl.add_method(
            objc::runtime::Sel::register("onClick:"),
            on_click as extern "C" fn(&objc::runtime::Object, objc::runtime::Sel, id),
        );

        let class = decl.register();
        ORB_DELEGATE_CLASS = class;
        &*class
    }
}

unsafe fn visible_frame() -> NSRect {
    let screen: id = msg_send![class!(NSScreen), mainScreen];
    if screen != nil {
        let frame: NSRect = msg_send![screen, visibleFrame];
        frame
    } else {
        NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(1440.0, 900.0))
    }
}

fn clamp_overlay_origin(
    mut x: f64,
    mut y: f64,
    width: f64,
    height: f64,
    frame: NSRect,
) -> (f64, f64) {
    let min_x = frame.origin.x + 6.0;
    let max_x = frame.origin.x + frame.size.width - width - 6.0;
    let min_y = frame.origin.y + 6.0;
    let max_y = frame.origin.y + frame.size.height - height - 6.0;

    if x < min_x {
        x = min_x;
    } else if x > max_x {
        x = max_x;
    }

    if y < min_y {
        y = min_y;
    } else if y > max_y {
        y = max_y;
    }

    (x, y)
}

fn point_in_frame(p: NSPoint, frame: NSRect) -> bool {
    p.x >= frame.origin.x
        && p.x <= frame.origin.x + frame.size.width
        && p.y >= frame.origin.y
        && p.y <= frame.origin.y + frame.size.height
}

fn is_cjk_char(ch: char) -> bool {
    matches!(
        ch as u32,
        0x4E00..=0x9FFF
            | 0x3400..=0x4DBF
            | 0x3000..=0x303F
            | 0x3040..=0x309F
            | 0x30A0..=0x30FF
            | 0xAC00..=0xD7AF
    )
}

fn preview_char_unit(ch: char) -> f32 {
    if ch.is_ascii_alphabetic() || ch.is_ascii_digit() {
        0.58
    } else if ch.is_ascii_punctuation() {
        0.42
    } else if is_cjk_char(ch) {
        1.0
    } else {
        0.72
    }
}

fn wrap_preview_text(raw: &str) -> String {
    let text = raw.replace('\r', "");
    if text.trim().is_empty() {
        return String::new();
    }

    let mut lines: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut width_units = 0.0f32;
    let mut truncated = false;

    for ch in text.chars() {
        let ch = if ch == '\t' { ' ' } else { ch };
        if ch == '\n' {
            lines.push(current);
            current = String::new();
            width_units = 0.0;
            if lines.len() >= OVERLAY_PREVIEW_MAX_LINES {
                truncated = true;
                break;
            }
            continue;
        }

        let unit = preview_char_unit(ch);
        if width_units + unit > OVERLAY_PREVIEW_LINE_CAP {
            lines.push(current);
            current = String::new();
            width_units = 0.0;
            if lines.len() >= OVERLAY_PREVIEW_MAX_LINES {
                truncated = true;
                break;
            }
        }

        current.push(ch);
        width_units += unit;
    }

    if !current.is_empty() && lines.len() < OVERLAY_PREVIEW_MAX_LINES {
        lines.push(current);
    } else if !current.is_empty() {
        truncated = true;
    }

    if lines.is_empty() {
        lines.push(String::new());
    }

    if truncated {
        if let Some(last) = lines.last_mut() {
            if !last.ends_with('…') {
                last.push('…');
            }
        }
    }

    lines.join("\n")
}

fn estimate_preview_lines(text: &str) -> usize {
    let cnt = text.lines().count();
    cnt.max(1).min(OVERLAY_PREVIEW_MAX_LINES)
}

unsafe fn layout_overlay_window(
    window: id,
    status_badge: id,
    status_label: id,
    preview_label: id,
    preview_text: &str,
) {
    let lines = estimate_preview_lines(preview_text);
    let preview_h = (OVERLAY_PREVIEW_LINE_HEIGHT * lines as f64).max(OVERLAY_PREVIEW_MIN_HEIGHT);
    let mut total_h = (preview_h + 18.0).max(OVERLAY_HEIGHT);
    if total_h > OVERLAY_MAX_HEIGHT {
        total_h = OVERLAY_MAX_HEIGHT;
    }

    let status_h = OVERLAY_STATUS_BADGE_HEIGHT;
    let status_w = OVERLAY_STATUS_BADGE_WIDTH;
    let badge_x = OVERLAY_STATUS_BADGE_X;
    let preview_x = badge_x + status_w + 16.0;
    let preview_w = OVERLAY_WIDTH - preview_x - 10.0;
    let status_y = ((total_h - status_h) * 0.5).floor();
    let preview_y = ((total_h - preview_h) * 0.5).floor();
    let badge_frame = NSRect::new(
        NSPoint::new(badge_x, status_y),
        NSSize::new(status_w, status_h),
    );
    let status_text_frame = NSRect::new(
        NSPoint::new(
            OVERLAY_STATUS_TEXT_X_OFFSET,
            status_y + OVERLAY_STATUS_TEXT_Y_OFFSET,
        ),
        NSSize::new(OVERLAY_WIDTH, status_h),
    );
    let preview_frame = NSRect::new(
        NSPoint::new(preview_x, preview_y),
        NSSize::new(preview_w, preview_h),
    );
    let _: () = msg_send![status_badge, setFrame: badge_frame];
    let _: () = msg_send![status_label, setFrame: status_text_frame];
    let _: () = msg_send![preview_label, setFrame: preview_frame];

    let current_frame: NSRect = msg_send![window, frame];
    if (current_frame.size.height - total_h).abs() > 0.5 {
        let resized = NSRect::new(current_frame.origin, NSSize::new(OVERLAY_WIDTH, total_h));
        let _: () = msg_send![window, setFrame: resized display: NO];
    }
}

unsafe fn focused_caret_rect() -> Option<AxRect> {
    if AXIsProcessTrusted() == 0 {
        return None;
    }

    let system = AXUIElementCreateSystemWide();
    if system.is_null() {
        return None;
    }

    let focused_attr = CFString::new("AXFocusedUIElement");
    let mut focused_val: core_foundation_sys::base::CFTypeRef = std::ptr::null();
    let copy_err =
        AXUIElementCopyAttributeValue(system, focused_attr.as_concrete_TypeRef(), &mut focused_val);
    CFRelease(system as core_foundation_sys::base::CFTypeRef);

    if copy_err != 0 || focused_val.is_null() {
        return None;
    }

    let focused = focused_val as AXUIElementRef;
    let range_attr = CFString::new("AXSelectedTextRange");
    let mut range_val: core_foundation_sys::base::CFTypeRef = std::ptr::null();
    let range_err =
        AXUIElementCopyAttributeValue(focused, range_attr.as_concrete_TypeRef(), &mut range_val);
    if range_err != 0 || range_val.is_null() {
        CFRelease(focused as core_foundation_sys::base::CFTypeRef);
        return None;
    }

    let bounds_attr = CFString::new("AXBoundsForRange");
    let mut bounds_val: core_foundation_sys::base::CFTypeRef = std::ptr::null();
    let bounds_err = AXUIElementCopyParameterizedAttributeValue(
        focused,
        bounds_attr.as_concrete_TypeRef(),
        range_val,
        &mut bounds_val,
    );
    CFRelease(range_val);
    CFRelease(focused as core_foundation_sys::base::CFTypeRef);

    if bounds_err != 0 || bounds_val.is_null() {
        return None;
    }

    let ax_value = bounds_val as AXValueRef;
    if AXValueGetType(ax_value) != K_AX_VALUE_CGRECT_TYPE {
        CFRelease(bounds_val);
        return None;
    }

    let mut rect = AxRect::default();
    let ok = AXValueGetValue(
        ax_value,
        K_AX_VALUE_CGRECT_TYPE,
        &mut rect as *mut _ as *mut c_void,
    );
    CFRelease(bounds_val);

    if ok == 0 {
        None
    } else {
        Some(rect)
    }
}

fn pick_focus_point(frame: NSRect, mouse: NSPoint, caret: AxRect) -> Option<NSPoint> {
    let center_x = caret.origin.x + caret.size.width * 0.5;
    let y_bottom_origin = caret.origin.y + caret.size.height * 0.5;
    let y_top_origin =
        frame.origin.y + frame.size.height - caret.origin.y - caret.size.height * 0.5;

    let candidates = [
        NSPoint::new(center_x, y_bottom_origin),
        NSPoint::new(center_x, y_top_origin),
    ];

    let mut best: Option<(f64, NSPoint)> = None;
    for point in candidates {
        if !point_in_frame(point, frame) {
            continue;
        }
        let dist2 = (point.x - mouse.x).powi(2) + (point.y - mouse.y).powi(2);
        match best {
            None => best = Some((dist2, point)),
            Some((curr, _)) if dist2 < curr => best = Some((dist2, point)),
            _ => {}
        }
    }
    best.map(|(_, p)| p)
}

// Returns true if positioned at top, false if at bottom
unsafe fn position_overlay_window(window: id) -> bool {
    let frame = visible_frame();
    let window_frame = NSWindow::frame(window);
    let width = window_frame.size.width;
    let height = window_frame.size.height;
    let x = frame.origin.x + (frame.size.width - width) * 0.5;
    let bottom_y = frame.origin.y + OVERLAY_BOTTOM_MARGIN;
    let top_y = frame.origin.y + frame.size.height - height - OVERLAY_TOP_MARGIN;
    let bottom_center = NSPoint::new(x + width * 0.5, bottom_y + height * 0.5);
    let mouse: NSPoint = msg_send![class!(NSEvent), mouseLocation];
    let focus = if let Some(caret) = focused_caret_rect() {
        pick_focus_point(frame, mouse, caret)
    } else if point_in_frame(mouse, frame) {
        Some(mouse)
    } else {
        None
    };
    let (y, is_top) = if let Some(p) = focus {
        let dx = p.x - bottom_center.x;
        let dy = p.y - bottom_center.y;
        if dx * dx + dy * dy <= OVERLAY_SWITCH_DISTANCE * OVERLAY_SWITCH_DISTANCE {
            (top_y, true)
        } else {
            (bottom_y, false)
        }
    } else {
        (bottom_y, false)
    };
    let (x, y) = clamp_overlay_origin(x, y, width, height, frame);
    window.setFrameOrigin_(NSPoint::new(x, y));
    is_top
}

unsafe fn install_overlay(show_orb: bool) -> Result<OverlayHandle> {
    let frame = visible_frame();
    let width = OVERLAY_WIDTH;
    let height = OVERLAY_HEIGHT;
    let x = frame.origin.x + (frame.size.width - width) / 2.0;
    let y = frame.origin.y + OVERLAY_BOTTOM_MARGIN;
    let rect = NSRect::new(NSPoint::new(x, y), NSSize::new(width, height));

    let window = NSWindow::alloc(nil).initWithContentRect_styleMask_backing_defer_(
        rect,
        NSWindowStyleMask::NSBorderlessWindowMask,
        NSBackingStoreBuffered,
        NO,
    );
    if window == nil {
        bail!("无法创建浮层窗口");
    }

    let clear_color: id = msg_send![class!(NSColor), clearColor];
    window.setBackgroundColor_(clear_color);
    window.setOpaque_(NO);
    window.setHasShadow_(YES);
    window.setIgnoresMouseEvents_(YES);
    window.setHidesOnDeactivate_(NO);
    window.setLevel_((NSMainMenuWindowLevel + 1) as i64);
    window.setCollectionBehavior_(
        NSWindowCollectionBehavior::NSWindowCollectionBehaviorCanJoinAllSpaces
            | NSWindowCollectionBehavior::NSWindowCollectionBehaviorTransient,
    );
    let _: () = msg_send![window, setReleasedWhenClosed: NO];
    let _: () = msg_send![window, setMovableByWindowBackground: NO];

    let content = window.contentView();
    if content == nil {
        bail!("浮层 contentView 为空");
    }
    let _: () = msg_send![content, setWantsLayer: YES];
    let content_layer: id = msg_send![content, layer];
    if content_layer != nil {
        let content_bg: id = msg_send![
            class!(NSColor),
            colorWithCalibratedWhite: 0.16f64
            alpha: 0.93f64
        ];
        let content_border: id = msg_send![
            class!(NSColor),
            colorWithCalibratedWhite: 0.44f64
            alpha: 0.34f64
        ];
        let content_bg_cg: id = msg_send![content_bg, CGColor];
        let content_border_cg: id = msg_send![content_border, CGColor];
        let _: () = msg_send![content_layer, setCornerRadius: 15.0f64];
        let _: () = msg_send![content_layer, setMasksToBounds: YES];
        let _: () = msg_send![content_layer, setBackgroundColor: content_bg_cg];
        let _: () = msg_send![content_layer, setBorderWidth: 1.0f64];
        let _: () = msg_send![content_layer, setBorderColor: content_border_cg];
    }

    let status_y = (OVERLAY_HEIGHT - OVERLAY_STATUS_BADGE_HEIGHT) * 0.5;
    let status_badge = NSView::initWithFrame_(
        NSView::alloc(nil),
        NSRect::new(
            NSPoint::new(OVERLAY_STATUS_BADGE_X, status_y),
            NSSize::new(OVERLAY_STATUS_BADGE_WIDTH, OVERLAY_STATUS_BADGE_HEIGHT),
        ),
    );
    let _: () = msg_send![status_badge, setWantsLayer: YES];
    let status_badge_layer: id = msg_send![status_badge, layer];
    if status_badge_layer != nil {
        let _: () = msg_send![
            status_badge_layer,
            setCornerRadius: (OVERLAY_STATUS_BADGE_HEIGHT * 0.5).floor()
        ];
        let _: () = msg_send![status_badge_layer, setMasksToBounds: YES];
    }
    content.addSubview_(status_badge);

    let status_label = NSTextField::initWithFrame_(
        NSTextField::alloc(nil),
        NSRect::new(
            NSPoint::new(
                OVERLAY_STATUS_TEXT_X_OFFSET,
                status_y + OVERLAY_STATUS_TEXT_Y_OFFSET,
            ),
            NSSize::new(OVERLAY_WIDTH, OVERLAY_STATUS_BADGE_HEIGHT),
        ),
    );
    let _: () = msg_send![status_label, setEditable: NO];
    let _: () = msg_send![status_label, setSelectable: NO];
    let _: () = msg_send![status_label, setBezeled: NO];
    let _: () = msg_send![status_label, setBordered: NO];
    let _: () = msg_send![status_label, setDrawsBackground: NO];
    let _: () = msg_send![status_label, setAlignment: 2usize];
    let status_font: id = msg_send![class!(NSFont), boldSystemFontOfSize: 13.0f64];
    let _: () = msg_send![status_label, setFont: status_font];
    let status_color: id = msg_send![class!(NSColor), whiteColor];
    let _: () = msg_send![status_label, setTextColor: status_color];
    let status_cell: id = msg_send![status_label, cell];
    if status_cell != nil {
        let _: () = msg_send![status_cell, setWraps: NO];
        let _: () = msg_send![status_cell, setScrollable: NO];
        let _: () = msg_send![status_cell, setUsesSingleLineMode: YES];
        let _: () = msg_send![status_cell, setLineBreakMode: 4usize];
        let _: () = msg_send![status_cell, setAlignment: 2usize];
    }
    let _: () = msg_send![status_label, setStringValue: ns_string("就绪")];
    set_status_badge_appearance(status_badge, "就绪");
    content.addSubview_(status_label);

    let preview_label = NSTextField::initWithFrame_(
        NSTextField::alloc(nil),
        NSRect::new(NSPoint::new(108.0, 15.0), NSSize::new(442.0, 20.0)),
    );
    let _: () = msg_send![preview_label, setEditable: NO];
    let _: () = msg_send![preview_label, setSelectable: NO];
    let _: () = msg_send![preview_label, setBezeled: NO];
    let _: () = msg_send![preview_label, setBordered: NO];
    let _: () = msg_send![preview_label, setDrawsBackground: NO];
    let _: () = msg_send![preview_label, setAlignment: 0usize];
    let preview_font: id = msg_send![class!(NSFont), systemFontOfSize: 15.0f64];
    let _: () = msg_send![preview_label, setFont: preview_font];
    let preview_color: id = msg_send![
        class!(NSColor),
        colorWithCalibratedRed: 0.94f64
        green: 0.91f64
        blue: 0.78f64
        alpha: 1.0f64
    ];
    let _: () = msg_send![preview_label, setTextColor: preview_color];
    let cell: id = msg_send![preview_label, cell];
    if cell != nil {
        let _: () = msg_send![cell, setWraps: YES];
        let _: () = msg_send![cell, setScrollable: NO];
        let _: () = msg_send![cell, setUsesSingleLineMode: NO];
        let _: () = msg_send![cell, setLineBreakMode: 0usize];
    }
    let _: () = msg_send![preview_label, setStringValue: ns_string("按住快捷键说话")];
    content.addSubview_(preview_label);

    window.orderOut_(nil);

    // Install history window
    let (history_window, item1, item2, item3, close_btn) = install_history_window()?;

    // Install floating orb (if enabled)
    let orb_window = if show_orb {
        let orb = install_floating_orb()?;
        unsafe {
            ORB_WINDOW_PTR = orb as usize;
        }
        orb
    } else {
        nil
    };

    Ok(OverlayHandle {
        window_ptr: window as usize,
        status_badge_ptr: status_badge as usize,
        status_label_ptr: status_label as usize,
        preview_label_ptr: preview_label as usize,
        history_window_ptr: history_window as usize,
        history_item1_ptr: item1 as usize,
        history_item2_ptr: item2 as usize,
        history_item3_ptr: item3 as usize,
        history_close_btn_ptr: close_btn as usize,
        orb_window_ptr: orb_window as usize,
    })
}

unsafe fn ns_string(s: &str) -> id {
    NSString::alloc(nil).init_str(s).autorelease()
}

unsafe fn set_status_badge_appearance(status_label: id, status: &str) {
    if status_label == nil {
        return;
    }
    let (r, g, b) = if status.contains("录音") {
        (0.20, 0.44, 0.95)
    } else if status.contains("转录") || status.contains("识别") {
        (0.35, 0.37, 0.44)
    } else if status.contains("润色") {
        (0.56, 0.43, 0.16)
    } else if status.contains("发送") || status.contains("注入") || status.contains("就绪") {
        (0.19, 0.42, 0.86)
    } else {
        (0.58, 0.24, 0.24)
    };
    let badge_bg: id = msg_send![
        class!(NSColor),
        colorWithCalibratedRed: r
        green: g
        blue: b
        alpha: 1.0f64
    ];
    let badge_bg_cg: id = msg_send![badge_bg, CGColor];
    let status_layer: id = msg_send![status_label, layer];
    if status_layer != nil {
        let bounds: NSRect = msg_send![status_label, bounds];
        let _: () = msg_send![
            status_layer,
            setCornerRadius: (bounds.size.height * 0.5).floor()
        ];
        let _: () = msg_send![status_layer, setMasksToBounds: YES];
        let _: () = msg_send![status_layer, setBackgroundColor: badge_bg_cg];
    }
}

unsafe fn set_status_button_symbol(button: id, symbol_name: &str) {
    let image: id = msg_send![
        class!(NSImage),
        imageWithSystemSymbolName: ns_string(symbol_name)
        accessibilityDescription: nil
    ];
    if image != nil {
        let _: () = msg_send![image, setTemplate: YES];
        NSButton::setImage_(button, image);
    }
}

// Position history window adjacent to the orb window (avoiding overlap)
unsafe fn position_history_window(window: id, _main_overlay_on_top: bool) {
    let screen_frame = visible_frame();
    let history_width = HISTORY_WIDTH;
    let history_height = HISTORY_HEIGHT;

    // Get orb window position
    let orb_window = ORB_WINDOW_PTR as id;
    if orb_window == nil {
        // Fallback to default position if orb not available
        let x = screen_frame.origin.x + screen_frame.size.width - history_width - HISTORY_MARGIN;
        let y = screen_frame.origin.y + HISTORY_MARGIN;
        window.setFrameOrigin_(NSPoint::new(x, y));
        return;
    }

    let orb_frame: NSRect = msg_send![orb_window, frame];

    // Gap between orb and history window
    let gap: f64 = 8.0;

    // Try positions in order: left, right, above, below
    let mut best_x = orb_frame.origin.x - history_width - gap;
    let mut best_y = orb_frame.origin.y + (orb_frame.size.height - history_height) * 0.5;

    // Check if left side has enough space
    if best_x < screen_frame.origin.x + 10.0 {
        // Not enough space on left, try right
        best_x = orb_frame.origin.x + orb_frame.size.width + gap;

        // Check if right side has enough space
        if best_x + history_width > screen_frame.origin.x + screen_frame.size.width - 10.0 {
            // Not enough space on right, try above
            best_x = orb_frame.origin.x + (orb_frame.size.width - history_width) * 0.5;
            best_y = orb_frame.origin.y + orb_frame.size.height + gap;

            // Check if above has enough space
            if best_y + history_height > screen_frame.origin.y + screen_frame.size.height - 10.0 {
                // Not enough space above, place below
                best_y = orb_frame.origin.y - history_height - gap;
            }
        }
    }

    // Clamp to screen bounds
    let (final_x, final_y) = clamp_overlay_origin(best_x, best_y, history_width, history_height, screen_frame);
    window.setFrameOrigin_(NSPoint::new(final_x, final_y));
}

// Create the history window with 3 items, copy buttons and close button
unsafe fn install_history_window() -> Result<(id, id, id, id, id)> {
    let frame = visible_frame();
    let rect = NSRect::new(
        NSPoint::new(0.0, 0.0),
        NSSize::new(HISTORY_WIDTH, HISTORY_HEIGHT),
    );

    let window = NSWindow::alloc(nil).initWithContentRect_styleMask_backing_defer_(
        rect,
        NSWindowStyleMask::NSBorderlessWindowMask,
        NSBackingStoreBuffered,
        NO,
    );
    if window == nil {
        bail!("无法创建历史窗口");
    }

    let clear_color: id = msg_send![class!(NSColor), clearColor];
    window.setBackgroundColor_(clear_color);
    window.setOpaque_(NO);
    window.setHasShadow_(YES);
    window.setIgnoresMouseEvents_(NO); // Allow mouse interaction
    window.setHidesOnDeactivate_(NO);
    window.setLevel_((NSMainMenuWindowLevel + 1) as i64);
    window.setCollectionBehavior_(
        NSWindowCollectionBehavior::NSWindowCollectionBehaviorCanJoinAllSpaces
            | NSWindowCollectionBehavior::NSWindowCollectionBehaviorTransient,
    );
    let _: () = msg_send![window, setReleasedWhenClosed: NO];
    let _: () = msg_send![window, setMovableByWindowBackground: YES];

    let content = window.contentView();
    if content == nil {
        bail!("历史窗口 contentView 为空");
    }
    let _: () = msg_send![content, setWantsLayer: YES];
    let content_layer: id = msg_send![content, layer];
    if content_layer != nil {
        let content_bg: id = msg_send![
            class!(NSColor),
            colorWithCalibratedWhite: 0.16f64
            alpha: 0.93f64
        ];
        let content_border: id = msg_send![
            class!(NSColor),
            colorWithCalibratedWhite: 0.44f64
            alpha: 0.34f64
        ];
        let content_bg_cg: id = msg_send![content_bg, CGColor];
        let content_border_cg: id = msg_send![content_border, CGColor];
        let _: () = msg_send![content_layer, setCornerRadius: 12.0f64];
        let _: () = msg_send![content_layer, setMasksToBounds: YES];
        let _: () = msg_send![content_layer, setBackgroundColor: content_bg_cg];
        let _: () = msg_send![content_layer, setBorderWidth: 1.0f64];
        let _: () = msg_send![content_layer, setBorderColor: content_border_cg];
    }

    // Title label
    let title_label = NSTextField::initWithFrame_(
        NSTextField::alloc(nil),
        NSRect::new(NSPoint::new(12.0, HISTORY_HEIGHT - 28.0), NSSize::new(200.0, 20.0)),
    );
    let _: () = msg_send![title_label, setEditable: NO];
    let _: () = msg_send![title_label, setSelectable: NO];
    let _: () = msg_send![title_label, setBezeled: NO];
    let _: () = msg_send![title_label, setBordered: NO];
    let _: () = msg_send![title_label, setDrawsBackground: NO];
    let title_font: id = msg_send![class!(NSFont), boldSystemFontOfSize: 12.0f64];
    let _: () = msg_send![title_label, setFont: title_font];
    let title_color: id = msg_send![class!(NSColor), colorWithCalibratedWhite: 0.7f64 alpha: 1.0f64];
    let _: () = msg_send![title_label, setTextColor: title_color];
    let _: () = msg_send![title_label, setStringValue: ns_string("最近输入")];
    content.addSubview_(title_label);

    // Close button
    let close_btn = NSButton::initWithFrame_(
        NSButton::alloc(nil),
        NSRect::new(
            NSPoint::new(HISTORY_WIDTH - 32.0, HISTORY_HEIGHT - 28.0),
            NSSize::new(20.0, 20.0),
        ),
    );
    let _: () = msg_send![close_btn, setBezelStyle: 8usize];
    let _: () = msg_send![close_btn, setBordered: NO];
    let _: () = msg_send![close_btn, setButtonType: 0usize];
    set_status_button_symbol(close_btn, "xmark");
    // Set up close action using a simple handler that hides the window
    let close_delegate = create_close_delegate(window);
    let _: () = msg_send![close_btn, setTarget: close_delegate];
    let _: () = msg_send![close_btn, setAction: sel!(closeHistory:)];
    content.addSubview_(close_btn);

    // Settings button (gear icon)
    let settings_btn = NSButton::initWithFrame_(
        NSButton::alloc(nil),
        NSRect::new(
            NSPoint::new(HISTORY_WIDTH - 58.0, HISTORY_HEIGHT - 28.0),
            NSSize::new(20.0, 20.0),
        ),
    );
    let _: () = msg_send![settings_btn, setBezelStyle: 8usize];
    let _: () = msg_send![settings_btn, setBordered: NO];
    let _: () = msg_send![settings_btn, setButtonType: 0usize];
    set_status_button_symbol(settings_btn, "gear");
    let settings_delegate = create_settings_delegate();
    let _: () = msg_send![settings_btn, setTarget: settings_delegate];
    let _: () = msg_send![settings_btn, setAction: sel!(openSettings:)];
    content.addSubview_(settings_btn);

    // Quit button (power icon)
    let quit_btn = NSButton::initWithFrame_(
        NSButton::alloc(nil),
        NSRect::new(
            NSPoint::new(HISTORY_WIDTH - 84.0, HISTORY_HEIGHT - 28.0),
            NSSize::new(20.0, 20.0),
        ),
    );
    let _: () = msg_send![quit_btn, setBezelStyle: 8usize];
    let _: () = msg_send![quit_btn, setBordered: NO];
    let _: () = msg_send![quit_btn, setButtonType: 0usize];
    set_status_button_symbol(quit_btn, "power");
    let quit_delegate = create_quit_delegate();
    let _: () = msg_send![quit_btn, setTarget: quit_delegate];
    let _: () = msg_send![quit_btn, setAction: sel!(quitApp:)];
    content.addSubview_(quit_btn);

    // Create 3 history items
    let item_width = HISTORY_WIDTH - 24.0;
    let item_x = 12.0;
    let copy_btn_width = 32.0;
    let text_width = item_width - copy_btn_width - 8.0;

    let mut item_ptrs: Vec<id> = Vec::new();

    for i in 0..3 {
        let y = HISTORY_HEIGHT - 58.0 - (i as f64 * HISTORY_ITEM_HEIGHT);

        // Text label
        let text_label = NSTextField::initWithFrame_(
            NSTextField::alloc(nil),
            NSRect::new(NSPoint::new(item_x, y), NSSize::new(text_width, 24.0)),
        );
        let _: () = msg_send![text_label, setEditable: NO];
        let _: () = msg_send![text_label, setSelectable: YES];
        let _: () = msg_send![text_label, setBezeled: NO];
        let _: () = msg_send![text_label, setBordered: NO];
        let _: () = msg_send![text_label, setDrawsBackground: NO];
        let text_font: id = msg_send![class!(NSFont), systemFontOfSize: 13.0f64];
        let _: () = msg_send![text_label, setFont: text_font];
        let text_color: id = msg_send![class!(NSColor), whiteColor];
        let _: () = msg_send![text_label, setTextColor: text_color];
        let _: () = msg_send![text_label, setLineBreakMode: 4usize]; // Truncate tail
        let _: () = msg_send![text_label, setStringValue: ns_string(&format!("占位文本 {}", i + 1))];
        content.addSubview_(text_label);
        item_ptrs.push(text_label);

        // Copy button
        let copy_btn = NSButton::initWithFrame_(
            NSButton::alloc(nil),
            NSRect::new(
                NSPoint::new(item_x + text_width + 4.0, y - 2.0),
                NSSize::new(copy_btn_width, 26.0),
            ),
        );
        let _: () = msg_send![copy_btn, setBezelStyle: 8usize]; // NSRoundRectBezelStyle
        let _: () = msg_send![copy_btn, setBordered: YES];
        let _: () = msg_send![copy_btn, setButtonType: 0usize]; // NSMomentaryLightButton
        set_status_button_symbol(copy_btn, "doc.on.doc");
        // Store index in tag for copy handler
        let _: () = msg_send![copy_btn, setTag: i as isize];
        let copy_delegate = create_copy_delegate(i);
        let _: () = msg_send![copy_btn, setTarget: copy_delegate];
        let _: () = msg_send![copy_btn, setAction: sel!(copyHistoryItem:)];
        content.addSubview_(copy_btn);
    }

    window.orderOut_(nil);

    Ok((window, item_ptrs[0], item_ptrs[1], item_ptrs[2], close_btn))
}

// Create floating orb window (常驻悬浮球)
unsafe fn install_floating_orb() -> Result<id> {
    let frame = visible_frame();
    let orb_size = ORB_SIZE;
    // Default position: bottom-right corner
    let x = frame.origin.x + frame.size.width - orb_size - ORB_MARGIN;
    let y = frame.origin.y + ORB_MARGIN;

    let rect = NSRect::new(
        NSPoint::new(x, y),
        NSSize::new(orb_size, orb_size),
    );

    let window = NSWindow::alloc(nil).initWithContentRect_styleMask_backing_defer_(
        rect,
        NSWindowStyleMask::NSBorderlessWindowMask,
        NSBackingStoreBuffered,
        NO,
    );
    if window == nil {
        bail!("无法创建悬浮球窗口");
    }

    let clear_color: id = msg_send![class!(NSColor), clearColor];
    window.setBackgroundColor_(clear_color);
    window.setOpaque_(NO);
    window.setHasShadow_(YES);
    window.setIgnoresMouseEvents_(NO); // Allow mouse interaction
    window.setHidesOnDeactivate_(NO);
    window.setLevel_((NSMainMenuWindowLevel + 1) as i64);
    window.setCollectionBehavior_(
        NSWindowCollectionBehavior::NSWindowCollectionBehaviorCanJoinAllSpaces
            | NSWindowCollectionBehavior::NSWindowCollectionBehaviorTransient,
    );
    let _: () = msg_send![window, setReleasedWhenClosed: NO];
    let _: () = msg_send![window, setMovableByWindowBackground: YES]; // Draggable

    let content = window.contentView();
    if content == nil {
        bail!("悬浮球 contentView 为空");
    }
    let _: () = msg_send![content, setWantsLayer: YES];
    let content_layer: id = msg_send![content, layer];
    if content_layer != nil {
        // Circular shape
        let _: () = msg_send![content_layer, setCornerRadius: orb_size * 0.5];
        let _: () = msg_send![content_layer, setMasksToBounds: YES];

        // Default background (idle state - blue)
        let orb_bg: id = msg_send![
            class!(NSColor),
            colorWithCalibratedRed: 0.20f64
            green: 0.44f64
            blue: 0.95f64
            alpha: 0.95f64
        ];
        let orb_bg_cg: id = msg_send![orb_bg, CGColor];
        let _: () = msg_send![content_layer, setBackgroundColor: orb_bg_cg];

        // Add icon as sublayer (doesn't block mouse events on the window background)
        let icon_layer: id = msg_send![class!(CALayer), layer];
        if icon_layer != nil {
            let icon_size = orb_size * 0.5;
            let icon_x = (orb_size - icon_size) * 0.5;
            let icon_y = (orb_size - icon_size) * 0.5;
            let icon_frame = NSRect::new(
                NSPoint::new(icon_x, icon_y),
                NSSize::new(icon_size, icon_size),
            );
            let _: () = msg_send![icon_layer, setFrame: icon_frame];

            // Load SF Symbol image
            let icon_image: id = msg_send![
                class!(NSImage),
                imageWithSystemSymbolName: ns_string("waveform")
                accessibilityDescription: nil
            ];
            if icon_image != nil {
                let _: () = msg_send![icon_image, setTemplate: YES];
                // Set image as layer contents
                let _: () = msg_send![icon_layer, setContents: icon_image];
            }

            // Add icon layer as sublayer
            let _: () = msg_send![content_layer, addSublayer: icon_layer];
        }
    }

    // Create click/drag handling view that covers entire window
    setup_orb_mouse_handling(window, content, orb_size);

    window.orderFrontRegardless();

    Ok(window)
}

// Mouse handling state
struct OrbDragState {
    is_dragging: bool,
    start_pos: (f64, f64),
    start_time: u64, // ms
    window_start_pos: (f64, f64),
}

static mut ORB_DRAG_STATE: Option<OrbDragState> = None;

fn current_time_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

// Track mouse events on orb window to distinguish click vs drag
unsafe fn setup_orb_mouse_handling(window: id, content: id, orb_size: f64) {
    // Create tracking view that will be the new content view
    let orb_tracking_class = register_orb_tracking_class();
    let tracking_view: id = msg_send![orb_tracking_class, alloc];
    let tracking_view: id = msg_send![
        tracking_view,
        initWithFrame: NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(orb_size, orb_size))
    ];

    if tracking_view != nil {
        // Store window reference in view
        let window_ptr = window as usize;
        (*tracking_view).set_ivar("orb_window_ptr", window_ptr);

        // Move original content view's subviews to tracking view
        let subviews: id = msg_send![content, subviews];
        let count: usize = msg_send![subviews, count];
        for i in 0..count {
            let subview: id = msg_send![subviews, objectAtIndex: 0usize]; // Always get first (index shifts)
            let _: () = msg_send![subview, removeFromSuperview];
            let _: () = msg_send![tracking_view, addSubview: subview];
        }

        // Copy layer settings from original content
        let _: () = msg_send![tracking_view, setWantsLayer: YES];
        let tracking_layer: id = msg_send![tracking_view, layer];
        if tracking_layer != nil {
            let content_layer: id = msg_send![content, layer];
            if content_layer != nil {
                // Copy corner radius
                let corner_radius: f64 = msg_send![content_layer, cornerRadius];
                let _: () = msg_send![tracking_layer, setCornerRadius: corner_radius];
                let _: () = msg_send![tracking_layer, setMasksToBounds: YES];

                // Copy background color
                let bg_color: id = msg_send![content_layer, backgroundColor];
                let _: () = msg_send![tracking_layer, setBackgroundColor: bg_color];

                // Copy sublayers (icon layer)
                let sublayers: id = msg_send![content_layer, sublayers];
                if sublayers != nil {
                    let count: usize = msg_send![sublayers, count];
                    for i in 0..count {
                        let sublayer: id = msg_send![sublayers, objectAtIndex: i];
                        let _: () = msg_send![tracking_layer, addSublayer: sublayer];
                    }
                }
            }
        }

        // Replace content view
        let _: () = msg_send![window, setContentView: tracking_view];
    }
}

fn register_orb_tracking_class() -> &'static objc::runtime::Class {
    use objc::declare::ClassDecl;
    use std::sync::Once;

    static mut CLASS: *const objc::runtime::Class = std::ptr::null();
    static INIT: Once = Once::new();

    INIT.call_once(|| {
        let superclass = objc::runtime::Class::get("NSView").unwrap();
        let mut decl = ClassDecl::new("OrbTrackingView", superclass).unwrap();

        decl.add_ivar::<usize>("orb_window_ptr");

        // mouseDown handler
        extern "C" fn mouse_down(this: &mut Object, _sel: Sel, event: id) {
            unsafe {
                let window_ptr: usize = *this.get_ivar("orb_window_ptr");
                let window = window_ptr as id;
                if window == nil {
                    return;
                }

                // Record start position
                let mouse_loc: NSPoint = msg_send![event, locationInWindow];
                let screen_mouse: NSPoint = msg_send![window, convertPointToScreen: mouse_loc];

                let window_frame: NSRect = msg_send![window, frame];

                ORB_DRAG_STATE = Some(OrbDragState {
                    is_dragging: true,
                    start_pos: (screen_mouse.x, screen_mouse.y),
                    start_time: current_time_ms(),
                    window_start_pos: (window_frame.origin.x, window_frame.origin.y),
                });
            }
        }

        // mouseDragged handler
        extern "C" fn mouse_dragged(this: &mut Object, _sel: Sel, event: id) {
            unsafe {
                let window_ptr: usize = *this.get_ivar("orb_window_ptr");
                let window = window_ptr as id;
                if window == nil {
                    return;
                }
                let state = match ORB_DRAG_STATE.as_ref() {
                    Some(s) if s.is_dragging => s,
                    _ => return,
                };

                let mouse_loc: NSPoint = msg_send![event, locationInWindow];
                let screen_mouse: NSPoint = msg_send![window, convertPointToScreen: mouse_loc];

                let dx = screen_mouse.x - state.start_pos.0;
                let dy = screen_mouse.y - state.start_pos.1;

                let new_x = state.window_start_pos.0 + dx;
                let new_y = state.window_start_pos.1 + dy;

                let _: () = msg_send![window, setFrameOrigin: NSPoint::new(new_x, new_y)];
            }
        }

        // mouseUp handler
        extern "C" fn mouse_up(_this: &mut Object, _sel: Sel, _event: id) {
            unsafe {
                let state = match ORB_DRAG_STATE.as_ref() {
                    Some(s) if s.is_dragging => s,
                    _ => return,
                };

                let elapsed = current_time_ms() - state.start_time;

                // If elapsed time < 200ms, treat as click
                if elapsed < 200 {
                    if let Some(ref tx) = ORB_CLICK_TX {
                        let _ = tx.send(OrbCommand::ToggleHistory);
                    }
                }

                // Reset state
                ORB_DRAG_STATE = None;
            }
        }

        unsafe {
            decl.add_method(
                sel!(mouseDown:),
                mouse_down as extern "C" fn(&mut Object, Sel, id),
            );
            decl.add_method(
                sel!(mouseDragged:),
                mouse_dragged as extern "C" fn(&mut Object, Sel, id),
            );
            decl.add_method(
                sel!(mouseUp:),
                mouse_up as extern "C" fn(&mut Object, Sel, id),
            );
        }

        let class = decl.register();
        unsafe { CLASS = class; }
    });

    unsafe { &*CLASS }
}

// Set up orb click handler
pub fn set_orb_click_handler(tx: std::sync::mpsc::Sender<OrbCommand>) {
    unsafe {
        ORB_CLICK_TX = Some(tx);
    }
}

// Create delegate for copy buttons
fn create_copy_delegate(index: usize) -> id {
    use objc::declare::ClassDecl;
    use std::sync::Once;

    static mut CLASS: *const objc::runtime::Class = std::ptr::null();
    static INIT: Once = Once::new();

    INIT.call_once(|| {
        let superclass = objc::runtime::Class::get("NSObject").unwrap();
        let mut decl = ClassDecl::new("HistoryCopyDelegate", superclass).unwrap();

        decl.add_ivar::<usize>("item_index");

        extern "C" fn copy_item(this: &mut Object, _sel: Sel, _sender: id) {
            unsafe {
                let index: usize = *this.get_ivar("item_index");
                let items = get_history_items();
                if let Some(text) = items.get(index) {
                    // Copy to clipboard
                    let pasteboard: id = msg_send![class!(NSPasteboard), generalPasteboard];
                    let _: () = msg_send![pasteboard, clearContents];
                    let ns_string = NSString::alloc(nil).init_str(text).autorelease();
                    let _: BOOL = msg_send![pasteboard, setString: ns_string forType: NSPasteboardTypeString];
                }
            }
        }

        unsafe {
            decl.add_method(
                sel!(copyHistoryItem:),
                copy_item as extern "C" fn(&mut Object, Sel, id),
            );
        }

        let class = decl.register();
        unsafe { CLASS = class; }
    });

    unsafe {
        let class = &*CLASS;
        let delegate: id = msg_send![class, alloc];
        let delegate: id = msg_send![delegate, init];
        (*delegate).set_ivar("item_index", index);
        delegate
    }
}

// Create delegate for quit button
fn create_quit_delegate() -> id {
    use objc::declare::ClassDecl;
    use std::sync::Once;

    static mut CLASS: *const objc::runtime::Class = std::ptr::null();
    static INIT: Once = Once::new();

    INIT.call_once(|| {
        let superclass = objc::runtime::Class::get("NSObject").unwrap();
        let mut decl = ClassDecl::new("QuitDelegate", superclass).unwrap();

        extern "C" fn quit_app(_this: &mut Object, _sel: Sel, _sender: id) {
            // Terminate the application
            unsafe {
                let app: id = msg_send![class!(NSApplication), sharedApplication];
                let _: () = msg_send![app, terminate: nil];
            }
        }

        unsafe {
            decl.add_method(
                sel!(quitApp:),
                quit_app as extern "C" fn(&mut Object, Sel, id),
            );
        }

        let class = decl.register();
        unsafe { CLASS = class; }
    });

    unsafe {
        let class = &*CLASS;
        let delegate: id = msg_send![class, alloc];
        let delegate: id = msg_send![delegate, init];
        delegate
    }
}

// Create delegate for settings button
fn create_settings_delegate() -> id {
    use objc::declare::ClassDecl;
    use std::sync::Once;

    static mut CLASS: *const objc::runtime::Class = std::ptr::null();
    static INIT: Once = Once::new();

    INIT.call_once(|| {
        let superclass = objc::runtime::Class::get("NSObject").unwrap();
        let mut decl = ClassDecl::new("SettingsDelegate", superclass).unwrap();

        extern "C" fn open_settings(_this: &mut Object, _sel: Sel, _sender: id) {
            // Call spawn_model_manager to open settings
            if let Err(e) = spawn_model_manager() {
                eprintln!("[mofa-ime] 打开设置失败: {e}");
            }
        }

        unsafe {
            decl.add_method(
                sel!(openSettings:),
                open_settings as extern "C" fn(&mut Object, Sel, id),
            );
        }

        let class = decl.register();
        unsafe { CLASS = class; }
    });

    unsafe {
        let class = &*CLASS;
        let delegate: id = msg_send![class, alloc];
        let delegate: id = msg_send![delegate, init];
        delegate
    }
}

// Create delegate for history window close button
fn create_close_delegate(window: id) -> id {
    use objc::declare::ClassDecl;
    use std::sync::Once;

    static mut CLASS: *const objc::runtime::Class = std::ptr::null();
    static INIT: Once = Once::new();

    INIT.call_once(|| {
        let superclass = objc::runtime::Class::get("NSObject").unwrap();
        let mut decl = ClassDecl::new("HistoryCloseDelegate", superclass).unwrap();

        decl.add_ivar::<usize>("window_ptr");

        extern "C" fn close_history(this: &mut Object, _sel: Sel, _sender: id) {
            unsafe {
                let window_ptr: usize = *this.get_ivar("window_ptr");
                let window = window_ptr as id;
                if window != nil {
                    let _: () = msg_send![window, orderOut: nil];
                }
            }
        }

        unsafe {
            decl.add_method(
                sel!(closeHistory:),
                close_history as extern "C" fn(&mut Object, Sel, id),
            );
        }

        let class = decl.register();
        unsafe { CLASS = class; }
    });

    unsafe {
        let class = &*CLASS;
        let delegate: id = msg_send![class, alloc];
        let delegate: id = msg_send![delegate, init];
        (*delegate).set_ivar("window_ptr", window as usize);
        delegate
    }
}
