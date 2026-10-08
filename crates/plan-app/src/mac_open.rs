//! macOS: opening plans from Finder.
//!
//! Double-clicking a `.psplan` (or dropping one on the Dock icon, or
//! `open -a "Plan Studio" file.psplan`) does not put the file on the command
//! line: Launch Services sends the running or starting app an "open
//! documents" Apple Event. winit 0.30 does not turn that event into anything,
//! so this module registers its own handler for it with the Apple Event
//! manager, through the Objective-C runtime (no extra crates), before the
//! event loop starts. The handler only queues the paths; the window takes them
//! with [`take_requests`] once a frame.
//!
//! AppKit installs its own handler for the same event while the app finishes
//! launching, which would replace ours on a cold start (the double-click that
//! starts the app), so the handler is registered again from the will-finish
//! and did-finish launching notifications. Checked by hand against a built
//! `.app`: `open -a "Plan Studio.app" file.psplan` opens the file both when
//! the app starts and when it is already running.
//!
//! Everything here runs on the main thread (the handler is called from the run
//! loop).

use std::ffi::{c_char, c_void, CStr};
use std::path::PathBuf;
use std::sync::atomic::{AtomicPtr, Ordering};
use std::sync::Mutex;

type Id = *mut c_void;
type Sel = *mut c_void;
type Class = *mut c_void;

#[link(name = "objc")]
extern "C" {
    fn objc_getClass(name: *const c_char) -> Class;
    fn objc_allocateClassPair(superclass: Class, name: *const c_char, extra: usize) -> Class;
    fn objc_registerClassPair(cls: Class);
    fn sel_registerName(name: *const c_char) -> Sel;
    fn class_addMethod(cls: Class, name: Sel, imp: *const c_void, types: *const c_char) -> bool;
    fn objc_msgSend();
}

/// A four-character Apple Event code.
const fn code(s: &[u8; 4]) -> u32 {
    u32::from_be_bytes(*s)
}

static QUEUE: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());
static REPAINT: Mutex<Option<eframe::egui::Context>> = Mutex::new(None);

/// Plans Finder asked to open since the last call.
pub fn take_requests() -> Vec<PathBuf> {
    QUEUE
        .lock()
        .map(|mut q| std::mem::take(&mut *q))
        .unwrap_or_default()
}

/// Lets the handler wake the window when a request arrives.
pub fn set_repaint(ctx: &eframe::egui::Context) {
    if let Ok(mut r) = REPAINT.lock() {
        if r.is_none() {
            *r = Some(ctx.clone());
        }
    }
}

fn sel(name: &CStr) -> Sel {
    // SAFETY: `name` is a valid NUL-terminated selector name.
    unsafe { sel_registerName(name.as_ptr()) }
}

/// `[recv sel]` returning an object.
unsafe fn send0(recv: Id, s: &CStr) -> Id {
    let f: unsafe extern "C" fn(Id, Sel) -> Id = std::mem::transmute(objc_msgSend as *const c_void);
    f(recv, sel(s))
}

/// The paths in an "open documents" event.
unsafe fn paths_of(event: Id) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if event.is_null() {
        return out;
    }
    // [event paramDescriptorForKeyword:keyDirectObject]
    let param: unsafe extern "C" fn(Id, Sel, u32) -> Id =
        std::mem::transmute(objc_msgSend as *const c_void);
    let list = param(event, sel(c"paramDescriptorForKeyword:"), code(b"----"));
    if list.is_null() {
        return out;
    }
    let count_fn: unsafe extern "C" fn(Id, Sel) -> isize =
        std::mem::transmute(objc_msgSend as *const c_void);
    let at_fn: unsafe extern "C" fn(Id, Sel, isize) -> Id =
        std::mem::transmute(objc_msgSend as *const c_void);
    let n = count_fn(list, sel(c"numberOfItems"));
    let items: Vec<Id> = if n > 0 {
        (1..=n)
            .map(|i| at_fn(list, sel(c"descriptorAtIndex:"), i))
            .collect()
    } else {
        vec![list]
    };
    for item in items {
        if item.is_null() {
            continue;
        }
        let url = send0(item, c"fileURLValue");
        if url.is_null() {
            continue;
        }
        let path = send0(url, c"path");
        if path.is_null() {
            continue;
        }
        let utf8 = send0(path, c"UTF8String") as *const c_char;
        if !utf8.is_null() {
            out.push(PathBuf::from(
                CStr::from_ptr(utf8).to_string_lossy().into_owned(),
            ));
        }
    }
    out
}

/// The Objective-C method: `-[PlanStudioOpenHandler handleOpen:withReplyEvent:]`.
extern "C" fn handle_open(_this: Id, _cmd: Sel, event: Id, _reply: Id) {
    // SAFETY: called by the Apple Event manager with a live event descriptor.
    let paths = unsafe { paths_of(event) };
    if paths.is_empty() {
        return;
    }
    if let Ok(mut q) = QUEUE.lock() {
        q.extend(paths);
    }
    if let Ok(r) = REPAINT.lock() {
        if let Some(ctx) = r.as_ref() {
            ctx.request_repaint();
        }
    }
}

static HANDLER: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());

#[link(name = "AppKit", kind = "framework")]
extern "C" {
    static NSApplicationWillFinishLaunchingNotification: Id;
    static NSApplicationDidFinishLaunchingNotification: Id;
}

/// Makes the handler object the Apple Event manager's handler for "open
/// documents". AppKit installs its own while the app finishes launching, so
/// this runs again from the launch notifications.
unsafe fn register_with_manager() {
    let handler = HANDLER.load(Ordering::Acquire);
    let manager_class = objc_getClass(c"NSAppleEventManager".as_ptr());
    if handler.is_null() || manager_class.is_null() {
        return;
    }
    let manager = send0(manager_class as Id, c"sharedAppleEventManager");
    if manager.is_null() {
        return;
    }
    let set: unsafe extern "C" fn(Id, Sel, Id, Sel, u32, u32) =
        std::mem::transmute(objc_msgSend as *const c_void);
    set(
        manager,
        sel(c"setEventHandler:andSelector:forEventClass:andEventID:"),
        handler,
        sel(c"handleOpen:withReplyEvent:"),
        code(b"aevt"),
        code(b"odoc"),
    );
}

/// `-[PlanStudioOpenHandler launching:]`, called by the launch notifications.
extern "C" fn launching(_this: Id, _cmd: Sel, _note: Id) {
    // SAFETY: main thread, handler registered by `install`.
    unsafe { register_with_manager() };
}

/// Registers the handler. Call once on the main thread, before the event
/// loop is created.
pub fn install() {
    // SAFETY: plain Objective-C runtime calls on the main thread with valid,
    // NUL-terminated names and the signatures documented by Apple.
    unsafe {
        let ns_object = objc_getClass(c"NSObject".as_ptr());
        let app_class = objc_getClass(c"NSApplication".as_ptr());
        let center_class = objc_getClass(c"NSNotificationCenter".as_ptr());
        if ns_object.is_null() || app_class.is_null() || center_class.is_null() {
            return;
        }
        let cls = objc_allocateClassPair(ns_object, c"PlanStudioOpenHandler".as_ptr(), 0);
        if cls.is_null() {
            return;
        }
        class_addMethod(
            cls,
            sel(c"handleOpen:withReplyEvent:"),
            handle_open as *const c_void,
            c"v@:@@".as_ptr(),
        );
        class_addMethod(
            cls,
            sel(c"launching:"),
            launching as *const c_void,
            c"v@:@".as_ptr(),
        );
        objc_registerClassPair(cls);
        // The application object must exist before its handlers are replaced.
        let _ = send0(app_class as Id, c"sharedApplication");
        let handler = send0(send0(cls as Id, c"alloc"), c"init");
        if handler.is_null() {
            return;
        }
        HANDLER.store(handler, Ordering::Release);
        register_with_manager();
        let center = send0(center_class as Id, c"defaultCenter");
        let observe: unsafe extern "C" fn(Id, Sel, Id, Sel, Id, Id) =
            std::mem::transmute(objc_msgSend as *const c_void);
        for name in [
            NSApplicationWillFinishLaunchingNotification,
            NSApplicationDidFinishLaunchingNotification,
        ] {
            observe(
                center,
                sel(c"addObserver:selector:name:object:"),
                handler,
                sel(c"launching:"),
                name,
                std::ptr::null_mut(),
            );
        }
    }
}
