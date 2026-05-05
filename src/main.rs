use objc2::sel;
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSMenu, NSMenuItem, NSStatusBar,
    NSVariableStatusItemLength,
};
use objc2_foundation::{MainThreadMarker, ns_string};

fn main() {
    let mtm = MainThreadMarker::new().expect("must run on main thread");

    let app = NSApplication::sharedApplication(mtm);
    app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);

    let status_bar = NSStatusBar::systemStatusBar();
    let status_item = status_bar.statusItemWithLength(NSVariableStatusItemLength);

    if let Some(button) = status_item.button(mtm) {
        button.setTitle(ns_string!("📋"));
    }

    let menu = NSMenu::new(mtm);
    let quit_item = NSMenuItem::new(mtm);
    quit_item.setTitle(ns_string!("Quit"));
    unsafe {
        quit_item.setAction(Some(sel!(terminate:)));
    }
    quit_item.setKeyEquivalent(ns_string!("q"));
    menu.addItem(&quit_item);

    status_item.setMenu(Some(&menu));

    app.run();
}
