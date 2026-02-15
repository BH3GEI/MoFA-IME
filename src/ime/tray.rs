#[derive(Clone, Copy)]
enum TrayState {
    Idle,
    Recording,
    Processing,
    Injected,
    Error,
}

impl TrayState {
    fn title(self) -> &'static str {
        match self {
            TrayState::Idle => "就绪",
            TrayState::Recording => "录音中",
            TrayState::Processing => "识别中",
            TrayState::Injected => "已发送",
            TrayState::Error => "失败",
        }
    }

    fn symbol_name(self) -> &'static str {
        match self {
            TrayState::Idle => "circle",
            TrayState::Recording => "mic.fill",
            TrayState::Processing => "hourglass",
            TrayState::Injected => "checkmark.circle.fill",
            TrayState::Error => "exclamationmark.triangle.fill",
        }
    }
}

#[derive(Clone, Copy)]
struct StatusHandle {
    button_ptr: usize,
}

impl StatusHandle {
    fn set(self, state: TrayState) {
        let button_ptr = self.button_ptr;
        let title = state.title().to_string();
        let symbol = state.symbol_name().to_string();
        Queue::main().exec_async(move || unsafe {
            let button = button_ptr as id;
            if button != nil {
                set_status_button_symbol(button, &symbol);
                NSButton::setTitle_(button, ns_string(&title));
            }
        });
    }
}

#[derive(Clone, Copy)]
struct MonitorHandle {
    state_item_ptr: usize,
    asr_item_ptr: usize,
    output_item_ptr: usize,
    hint_item_ptr: usize,
}

impl MonitorHandle {
    fn set_state(self, text: &str) {
        self.set_item(self.state_item_ptr, "状态", text);
    }

    fn set_asr(self, text: &str) {
        self.set_item(self.asr_item_ptr, "识别", text);
    }

    fn set_output(self, text: &str) {
        self.set_item(self.output_item_ptr, "发送", text);
    }

    fn set_hint(self, text: &str) {
        self.set_item(self.hint_item_ptr, "提示", text);
    }

    fn set_item(self, item_ptr: usize, label: &str, value: &str) {
        let title = format!("{label}: {}", truncate_middle(value, 64));
        Queue::main().exec_async(move || unsafe {
            let item = item_ptr as id;
            if item != nil {
                let _: () = msg_send![item, setTitle: ns_string(&title)];
            }
        });
    }
}

#[derive(Clone, Copy)]
struct OverlayHandle {
    window_ptr: usize,
    status_badge_ptr: usize,
    status_label_ptr: usize,
    preview_label_ptr: usize,
    // History window
    history_window_ptr: usize,
    history_scroll_view_ptr: usize,
    history_list_view_ptr: usize,
    history_close_btn_ptr: usize,
    // Floating orb (常驻悬浮球)
    orb_window_ptr: usize,
}

impl OverlayHandle {
    fn show_recording(self) {
        self.show("录音中", "请说话，松开快捷键结束");
    }

    fn show_transcribing(self) {
        self.show("转录中", "语音识别进行中");
    }

    fn show_refining(self) {
        self.update(true, Some("润色中".to_string()), None);
    }

    fn show_injected(self) {
        self.show("已发送", "文本已写入目标输入框");
    }

    fn show_error(self, message: &str) {
        self.show("失败了", message);
    }

    fn set_status(self, text: &str) {
        self.update(true, Some(text.to_string()), None);
    }

    fn set_preview(self, text: &str) {
        let line = wrap_preview_text(text);
        self.update(true, None, Some(line));
    }

    fn hide(self) {
        self.update(false, None, None);
    }

    fn fade_out_quick(self) {
        let window_ptr = self.window_ptr;
        let step_ms = (OVERLAY_FADE_TOTAL_MS / OVERLAY_FADE_STEPS.max(1)).max(1);
        for idx in (0..OVERLAY_FADE_STEPS).rev() {
            let alpha = idx as f64 / OVERLAY_FADE_STEPS as f64;
            Queue::main().exec_sync(move || unsafe {
                let window = window_ptr as id;
                if window != nil {
                    let _: () = msg_send![window, setAlphaValue: alpha];
                }
            });
            std::thread::sleep(Duration::from_millis(step_ms));
        }
        Queue::main().exec_sync(move || unsafe {
            let window = window_ptr as id;
            if window != nil {
                window.orderOut_(nil);
                let _: () = msg_send![window, setAlphaValue: 1.0f64];
            }
        });
    }

    fn show(self, status: &str, preview: &str) {
        self.update(
            true,
            Some(status.to_string()),
            Some(wrap_preview_text(preview)),
        );
    }

    fn update(self, visible: bool, status: Option<String>, preview: Option<String>) {
        let window_ptr = self.window_ptr;
        let status_badge_ptr = self.status_badge_ptr;
        let status_ptr = self.status_label_ptr;
        let preview_ptr = self.preview_label_ptr;
        Queue::main().exec_async(move || unsafe {
            let window = window_ptr as id;
            if window == nil {
                return;
            }
            let preview_for_layout = preview.map(|p| wrap_preview_text(&p));

            if let Some(s) = status {
                let status_badge = status_badge_ptr as id;
                let status_label = status_ptr as id;
                if status_label != nil {
                    let _: () = msg_send![status_label, setStringValue: ns_string(&s)];
                }
                if status_badge != nil {
                    set_status_badge_appearance(status_badge, &s);
                }
            }

            if let Some(p) = preview_for_layout.as_ref() {
                let preview_label = preview_ptr as id;
                if preview_label != nil {
                    let _: () = msg_send![preview_label, setStringValue: ns_string(p)];
                }
            }

            let preview_label = preview_ptr as id;
            let status_badge = status_badge_ptr as id;
            let status_label = status_ptr as id;
            if preview_label != nil && status_label != nil && status_badge != nil {
                let preview_text = if let Some(current) = preview_for_layout.as_ref() {
                    current.clone()
                } else {
                    let preview_ns: id = msg_send![preview_label, stringValue];
                    nsstring_to_rust(preview_ns).unwrap_or_default()
                };
                layout_overlay_window(
                    window,
                    status_badge,
                    status_label,
                    preview_label,
                    &preview_text,
                );
            }

            if visible {
                let _is_top = position_overlay_window(window);
                let _: () = msg_send![window, setAlphaValue: 1.0f64];
                window.orderFrontRegardless();
            } else {
                window.orderOut_(nil);
                let _: () = msg_send![window, setAlphaValue: 1.0f64];
            }
        });
    }

    fn show_history_internal(self, position_top: bool, auto_hide: bool) {
        let history_window_ptr = self.history_window_ptr;
        let history_scroll_view_ptr = self.history_scroll_view_ptr;
        let history_list_view_ptr = self.history_list_view_ptr;
        let _close_btn_ptr = self.history_close_btn_ptr;

        // Get history items
        let history = get_history_items();

        Queue::main().exec_async(move || unsafe {
            let window = history_window_ptr as id;
            if window == nil {
                return;
            }

            let scroll_view = history_scroll_view_ptr as id;
            let list_view = history_list_view_ptr as id;
            rebuild_history_list_view(scroll_view, list_view, &history, true);

            // Position history window same side as main overlay
            position_history_window(window, position_top);

            let _: () = msg_send![window, setAlphaValue: 1.0f64];
            window.orderFrontRegardless();

            // Auto-hide only if enabled (for overlay-triggered history)
            if auto_hide {
                let window_ptr_for_timer = history_window_ptr;
                std::thread::spawn(move || {
                    let mut elapsed_ms = 0u64;
                    let check_interval = Duration::from_millis(200);
                    let timeout_ms = 8000u64;

                    while elapsed_ms < timeout_ms {
                        std::thread::sleep(check_interval);
                        elapsed_ms += 200;

                        // Check if mouse is over the window
                        let should_hide = Queue::main().exec_sync(move || unsafe {
                            let w = window_ptr_for_timer as id;
                            if w == nil {
                                return true; // Window gone, hide
                            }

                            // Get window frame
                            let window_frame: NSRect = msg_send![w, frame];

                            // Get mouse location (screen coordinates)
                            let mouse_loc: NSPoint = msg_send![class!(NSEvent), mouseLocation];

                            // Check if mouse is inside window frame
                            let is_inside = mouse_loc.x >= window_frame.origin.x
                                && mouse_loc.x <= window_frame.origin.x + window_frame.size.width
                                && mouse_loc.y >= window_frame.origin.y
                                && mouse_loc.y <= window_frame.origin.y + window_frame.size.height;

                            !is_inside // Return true (should hide) if mouse is NOT inside
                        });

                        if should_hide {
                            break;
                        }
                        // If mouse is inside, reset timer
                        elapsed_ms = 0;
                    }

                    // Hide the window
                    Queue::main().exec_async(move || unsafe {
                        let w = window_ptr_for_timer as id;
                        if w != nil {
                            w.orderOut_(nil);
                        }
                    });
                });
            }
        });
    }

    fn show_history(self, position_top: bool) {
        // Called from orb click - don't auto hide
        self.show_history_internal(position_top, false);
    }

    fn hide_history(self) {
        self.hide_history_internal();
    }

    fn refresh_history_if_visible(self) {
        let history_window_ptr = self.history_window_ptr;
        let history_scroll_view_ptr = self.history_scroll_view_ptr;
        let history_list_view_ptr = self.history_list_view_ptr;

        // Get history items
        let history = get_history_items();

        Queue::main().exec_async(move || unsafe {
            let window = history_window_ptr as id;
            if window == nil {
                return;
            }

            // Check if window is visible
            let is_visible: i8 = msg_send![window, isVisible];
            if is_visible == 0 {
                return; // Window not visible, no need to refresh
            }

            let scroll_view = history_scroll_view_ptr as id;
            let list_view = history_list_view_ptr as id;
            // New history items are inserted at index 0; keep latest entry visible.
            rebuild_history_list_view(scroll_view, list_view, &history, true);
        });
    }

    fn hide_history_internal(self) {
        let history_window_ptr = self.history_window_ptr;
        Queue::main().exec_async(move || unsafe {
            let window = history_window_ptr as id;
            if window != nil {
                window.orderOut_(nil);
                let _: () = msg_send![window, setAlphaValue: 1.0f64];
            }
        });
    }

    fn show_orb(self) {
        let orb_window_ptr = self.orb_window_ptr;
        Queue::main().exec_async(move || unsafe {
            let window = orb_window_ptr as id;
            if window != nil {
                window.orderFrontRegardless();
            }
        });
    }

    fn hide_orb(self) {
        let orb_window_ptr = self.orb_window_ptr;
        Queue::main().exec_async(move || unsafe {
            let window = orb_window_ptr as id;
            if window != nil {
                window.orderOut_(nil);
            }
        });
    }
}

unsafe fn rebuild_history_list_view(scroll_view: id, list_view: id, history: &[String], scroll_to_top: bool) {
    if scroll_view == nil || list_view == nil {
        return;
    }

    // Clear existing rows.
    // `subviews` may be a snapshot-like array; remove from the end to avoid stale index reuse.
    loop {
        let subviews: id = msg_send![list_view, subviews];
        let count: usize = msg_send![subviews, count];
        if count == 0 {
            break;
        }
        let subview: id = msg_send![subviews, objectAtIndex: count - 1];
        let _: () = msg_send![subview, removeFromSuperview];
    }

    let scroll_frame: NSRect = msg_send![scroll_view, frame];
    let visible_height = scroll_frame.size.height.max(HISTORY_ITEM_HEIGHT);
    let content_width = (scroll_frame.size.width - 4.0).max(120.0);
    let row_height = HISTORY_ITEM_HEIGHT.max(28.0);
    let row_count = history.len().max(1);
    let doc_height = (row_count as f64 * row_height).max(visible_height);
    let _: () = msg_send![
        list_view,
        setFrame: NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(content_width, doc_height))
    ];

    if history.is_empty() {
        let empty_label = NSTextField::initWithFrame_(
            NSTextField::alloc(nil),
            NSRect::new(
                NSPoint::new(2.0, doc_height - row_height + 4.0),
                NSSize::new(content_width - 4.0, 24.0),
            ),
        );
        let _: () = msg_send![empty_label, setEditable: NO];
        let _: () = msg_send![empty_label, setSelectable: NO];
        let _: () = msg_send![empty_label, setBezeled: NO];
        let _: () = msg_send![empty_label, setBordered: NO];
        let _: () = msg_send![empty_label, setDrawsBackground: NO];
        let text_font: id = msg_send![class!(NSFont), systemFontOfSize: 13.0f64];
        let _: () = msg_send![empty_label, setFont: text_font];
        let text_color: id = msg_send![class!(NSColor), colorWithCalibratedWhite: 0.72f64 alpha: 1.0f64];
        let _: () = msg_send![empty_label, setTextColor: text_color];
        let _: () = msg_send![empty_label, setLineBreakMode: 4usize];
        let _: () = msg_send![empty_label, setStringValue: ns_string("（无）")];
        let cell: id = msg_send![empty_label, cell];
        if cell != nil {
            let _: () = msg_send![cell, setAlignment: 1usize];
        }
        let _: () = msg_send![list_view, addSubview: empty_label];
    } else {
        let copy_delegate = create_copy_delegate();
        let copy_btn_width = 32.0;
        let text_width = (content_width - copy_btn_width - 8.0).max(72.0);

        for (i, text) in history.iter().enumerate() {
            let row_y = doc_height - ((i as f64 + 1.0) * row_height);
            let text_label = NSTextField::initWithFrame_(
                NSTextField::alloc(nil),
                NSRect::new(NSPoint::new(0.0, row_y + 4.0), NSSize::new(text_width, 24.0)),
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
            let _: () = msg_send![text_label, setLineBreakMode: 4usize];
            let _: () = msg_send![text_label, setStringValue: ns_string(&truncate(text, 80))];
            let _: () = msg_send![list_view, addSubview: text_label];

            let copy_btn = NSButton::initWithFrame_(
                NSButton::alloc(nil),
                NSRect::new(
                    NSPoint::new(text_width + 4.0, row_y + 2.0),
                    NSSize::new(copy_btn_width, 26.0),
                ),
            );
            let _: () = msg_send![copy_btn, setBezelStyle: 8usize];
            let _: () = msg_send![copy_btn, setBordered: YES];
            let _: () = msg_send![copy_btn, setButtonType: 0usize];
            set_status_button_symbol(copy_btn, "doc.on.doc");
            let _: () = msg_send![copy_btn, setTag: i as isize];
            let _: () = msg_send![copy_btn, setTarget: copy_delegate];
            let _: () = msg_send![copy_btn, setAction: sel!(copyHistoryItem:)];
            let _: () = msg_send![list_view, addSubview: copy_btn];
        }
    }

    if scroll_to_top {
        let clip_view: id = msg_send![scroll_view, contentView];
        if clip_view != nil {
            let is_flipped: BOOL = msg_send![list_view, isFlipped];
            let top_y = if is_flipped == YES {
                0.0
            } else {
                (doc_height - visible_height).max(0.0)
            };
            let _: () = msg_send![clip_view, scrollToPoint: NSPoint::new(0.0, top_y)];
            let _: () = msg_send![scroll_view, reflectScrolledClipView: clip_view];
        }
    }
}

fn truncate_middle(s: &str, max_chars: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max_chars {
        return s.to_string();
    }
    if max_chars < 8 {
        return chars.into_iter().take(max_chars).collect();
    }
    let head = (max_chars - 1) / 2;
    let tail = max_chars - 1 - head;
    let mut out = String::new();
    out.extend(chars[..head].iter());
    out.push('…');
    out.extend(chars[chars.len() - tail..].iter());
    out
}

fn truncate(s: &str, max_chars: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max_chars {
        return s.to_string();
    }
    let mut out: String = chars.into_iter().take(max_chars - 1).collect();
    out.push('…');
    out
}

unsafe fn make_info_item(title: &str, target: id) -> id {
    let item = NSMenuItem::alloc(nil)
        .initWithTitle_action_keyEquivalent_(ns_string(title), sel!(noopInfo:), ns_string(""))
        .autorelease();
    NSMenuItem::setTarget_(item, target);
    let _: () = msg_send![item, setEnabled: NO];
    item
}

extern "C" fn open_model_manager_action(_this: &Object, _cmd: Sel, _sender: id) {
    if let Err(e) = spawn_model_manager() {
        eprintln!("[mofa-ime] 打开模型管理器失败: {e}");
    }
}

extern "C" fn noop_info_action(_this: &Object, _cmd: Sel, _sender: id) {}

fn menu_handler_class() -> *const Class {
    static CLS: OnceLock<usize> = OnceLock::new();
    *CLS.get_or_init(|| unsafe {
        let superclass = class!(NSObject);
        let mut decl =
            ClassDecl::new("MofaMenuHandler", superclass).expect("failed to declare class");
        decl.add_method(
            sel!(openModelManager:),
            open_model_manager_action as extern "C" fn(&Object, Sel, id),
        );
        decl.add_method(
            sel!(noopInfo:),
            noop_info_action as extern "C" fn(&Object, Sel, id),
        );
        (decl.register() as *const Class) as usize
    }) as *const Class
}

unsafe fn new_menu_handler() -> id {
    let cls = menu_handler_class();
    let obj: id = msg_send![cls, new];
    obj
}

fn spawn_model_manager() -> Result<()> {
    let exe = std::env::current_exe().context("无法获取当前可执行文件路径")?;
    let bin_dir = exe.parent().ok_or_else(|| anyhow!("无法获取可执行目录"))?;
    let project_dir = bin_dir
        .parent()
        .and_then(|p| p.parent())
        .ok_or_else(|| anyhow!("无法推断项目目录"))?;
    let cargo_toml = project_dir.join("Cargo.toml");
    if cfg!(debug_assertions) && cargo_toml.exists() {
        Command::new("cargo")
            .args(["run", "--offline", "--bin", "model-manager"])
            .current_dir(project_dir)
            .spawn()
            .context("以 cargo 启动 model-manager 失败")?;
        return Ok(());
    }

    let manager_bin = bin_dir.join("model-manager");
    if manager_bin.exists() {
        Command::new(manager_bin)
            .spawn()
            .context("启动 model-manager 失败")?;
        return Ok(());
    }

    if cargo_toml.exists() {
        Command::new("cargo")
            .args(["run", "--offline", "--bin", "model-manager"])
            .current_dir(project_dir)
            .spawn()
            .context("以 cargo 启动 model-manager 失败")?;
        return Ok(());
    }

    bail!("未找到 model-manager 可执行文件");
}

unsafe fn install_status_item(app: id) -> Result<(StatusHandle, MonitorHandle, id, id, id)> {
    let status_bar = NSStatusBar::systemStatusBar(nil);
    if status_bar == nil {
        bail!("无法创建 NSStatusBar");
    }

    let status_item = status_bar.statusItemWithLength_(NSVariableStatusItemLength);
    if status_item == nil {
        bail!("无法创建 status item");
    }

    let button = status_item.button();
    if button == nil {
        bail!("status item 无按钮");
    }
    NSButton::setTitle_(button, ns_string(TrayState::Idle.title()));
    set_status_button_symbol(button, TrayState::Idle.symbol_name());

    let menu = NSMenu::new(nil).autorelease();
    let menu_handler = new_menu_handler();
    let state_item = make_info_item("状态: 就绪", menu_handler);
    let asr_item = make_info_item("识别: -", menu_handler);
    let output_item = make_info_item("发送: -", menu_handler);
    let hint_item = make_info_item("提示: -", menu_handler);

    menu.addItem_(state_item);
    menu.addItem_(asr_item);
    menu.addItem_(output_item);
    menu.addItem_(hint_item);
    menu.addItem_(NSMenuItem::separatorItem(nil));

    let settings_item = NSMenuItem::alloc(nil)
        .initWithTitle_action_keyEquivalent_(
            ns_string("MoFA IME 设置..."),
            sel!(openModelManager:),
            ns_string("s"),
        )
        .autorelease();
    NSMenuItem::setTarget_(settings_item, menu_handler);
    menu.addItem_(settings_item);

    menu.addItem_(NSMenuItem::separatorItem(nil));

    let quit_item = NSMenuItem::alloc(nil)
        .initWithTitle_action_keyEquivalent_(ns_string("退出"), sel!(terminate:), ns_string("q"))
        .autorelease();
    NSMenuItem::setTarget_(quit_item, app);
    menu.addItem_(quit_item);
    status_item.setMenu_(menu);

    Ok((
        StatusHandle {
            button_ptr: button as usize,
        },
        MonitorHandle {
            state_item_ptr: state_item as usize,
            asr_item_ptr: asr_item as usize,
            output_item_ptr: output_item as usize,
            hint_item_ptr: hint_item as usize,
        },
        status_item,
        menu,
        menu_handler,
    ))
}
