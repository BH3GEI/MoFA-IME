fn inject_text(text: &str) -> Result<()> {
    if text.trim().is_empty() {
        return Ok(());
    }

    if unsafe { AXIsProcessTrusted() } == 0 {
        bail!("未授予辅助功能权限，请在 系统设置 -> 隐私与安全性 -> 辅助功能 中允许 MoFA IME");
    }

    // 直接在调用线程执行，避免队列排队导致的剪贴板竞争
    // 注意：所有 UI 相关操作都已在主线程运行（通过管道事件触发）
    let _pool = unsafe { NSAutoreleasePool::new(nil) };

    // 剪贴板粘贴重试两次，提升兼容性。
    for _ in 0..2 {
        if paste_via_clipboard(text).is_ok() {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(90));
    }

    Err(anyhow!("剪贴板粘贴失败"))
}

type AXUIElementRef = *const c_void;
type AXError = i32;

#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn AXIsProcessTrusted() -> core_foundation_sys::base::Boolean;
    fn AXUIElementCreateSystemWide() -> AXUIElementRef;
    fn AXUIElementCopyAttributeValue(
        element: AXUIElementRef,
        attribute: core_foundation_sys::string::CFStringRef,
        value: *mut core_foundation_sys::base::CFTypeRef,
    ) -> AXError;
    fn AXUIElementCopyParameterizedAttributeValue(
        element: AXUIElementRef,
        parameterized_attribute: core_foundation_sys::string::CFStringRef,
        parameter: core_foundation_sys::base::CFTypeRef,
        value: *mut core_foundation_sys::base::CFTypeRef,
    ) -> AXError;
    fn AXValueGetType(value: AXValueRef) -> AXValueType;
    fn AXValueGetValue(
        value: AXValueRef,
        value_type: AXValueType,
        value_ptr: *mut c_void,
    ) -> core_foundation_sys::base::Boolean;
}

fn paste_via_clipboard(text: &str) -> Result<()> {
    unsafe {
        let pboard: id = NSPasteboard::generalPasteboard(nil);
        if pboard == nil {
            bail!("无法获取 NSPasteboard");
        }

        // 不保存/恢复旧剪贴板，避免覆盖用户在此期间复制的内容
        pboard.clearContents();
        // 等待剪贴板清空完成
        std::thread::sleep(Duration::from_millis(20));

        let new_text = NSString::alloc(nil).init_str(text).autorelease();
        let ok = pboard.setString_forType(new_text, NSPasteboardTypeString);
        if !ok {
            bail!("写入剪贴板失败");
        }
        // 等待剪贴板同步完成，避免粘贴旧内容
        std::thread::sleep(Duration::from_millis(30));

        post_cmd_v()?;

        // 增加等待时间，提升在慢速应用（如终端）中的成功率
        std::thread::sleep(Duration::from_millis(350));

        Ok(())
    }
}

unsafe fn nsstring_to_rust(s: id) -> Option<String> {
    if s == nil {
        return None;
    }
    let ptr = s.UTF8String();
    if ptr.is_null() {
        return None;
    }
    Some(CStr::from_ptr(ptr).to_string_lossy().into_owned())
}

fn post_cmd_v() -> Result<()> {
    const KEY_V: CGKeyCode = 0x09;

    let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState)
        .map_err(|_| anyhow!("创建 CGEventSource 失败"))?;

    let cmd_down = CGEvent::new_keyboard_event(source.clone(), KeyCode::COMMAND, true)
        .map_err(|_| anyhow!("创建 cmd down 失败"))?;
    cmd_down.post(CGEventTapLocation::HID);

    let v_down = CGEvent::new_keyboard_event(source.clone(), KEY_V, true)
        .map_err(|_| anyhow!("创建 v down 失败"))?;
    v_down.set_flags(CGEventFlags::CGEventFlagCommand);
    v_down.post(CGEventTapLocation::HID);

    let v_up = CGEvent::new_keyboard_event(source.clone(), KEY_V, false)
        .map_err(|_| anyhow!("创建 v up 失败"))?;
    v_up.set_flags(CGEventFlags::CGEventFlagCommand);
    v_up.post(CGEventTapLocation::HID);

    let cmd_up = CGEvent::new_keyboard_event(source, KeyCode::COMMAND, false)
        .map_err(|_| anyhow!("创建 cmd up 失败"))?;
    cmd_up.post(CGEventTapLocation::HID);

    Ok(())
}
