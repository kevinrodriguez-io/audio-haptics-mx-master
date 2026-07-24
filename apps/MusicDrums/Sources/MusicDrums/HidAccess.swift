import AppKit
import Foundation
import IOKit.hid

enum HidAccess {
    enum Status: String {
        case granted
        case denied
        case unknown
    }

    static func check() -> Status {
        switch IOHIDCheckAccess(kIOHIDRequestTypeListenEvent) {
        case kIOHIDAccessTypeGranted:
            return .granted
        case kIOHIDAccessTypeDenied:
            return .denied
        default:
            return .unknown
        }
    }

    /// Shows the system Input Monitoring prompt when possible.
    @discardableResult
    static func requestIfNeeded() -> Status {
        let current = check()
        if current == .granted {
            return .granted
        }
        _ = IOHIDRequestAccess(kIOHIDRequestTypeListenEvent)
        return check()
    }

    static var permissionHelp: String {
        """
        macOS is blocking HID (Input Monitoring).

        1. System Settings → Privacy & Security → Input Monitoring
        2. Enable MusicDrums (remove + re-add if the toggle is already on after a rebuild)
        3. Quit MusicDrums fully, then reopen

        Terminal can work while the app fails — they are separate permissions.
        """
    }

    static func openSystemSettings() {
        let urls = [
            "x-apple.systempreferences:com.apple.preference.security?Privacy_ListenEvent",
            "x-apple.systempreferences:com.apple.settings.PrivacySecurity.extension?Privacy_ListenEvent",
        ]
        for s in urls {
            if let url = URL(string: s), NSWorkspace.shared.open(url) {
                return
            }
        }
    }
}
