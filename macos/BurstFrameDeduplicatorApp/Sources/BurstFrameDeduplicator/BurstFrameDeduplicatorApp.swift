import AppKit
import BurstFrameAppCore
import SwiftUI

private final class AppLifecycleDelegate: NSObject, NSApplicationDelegate {
    static weak var current: AppLifecycleDelegate?

    var prepareForTermination: ((@escaping () -> Void) -> Void)?
    private var quitEventMonitor: Any?
    private var terminationPending = false

    override init() {
        super.init()
        Self.current = self
    }

    func applicationDidFinishLaunching(_ notification: Notification) {
        quitEventMonitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { [weak self] event in
            let modifiers = event.modifierFlags.intersection(.deviceIndependentFlagsMask)
            let isQuitKey = event.charactersIgnoringModifiers?.lowercased() == "q"
            let hasExtraShortcutModifier = !modifiers
                .intersection([.control, .option, .shift])
                .isEmpty
            guard modifiers.contains(.command), !hasExtraShortcutModifier, isQuitKey
            else { return event }
            self?.requestTermination(NSApplication.shared)
            return nil
        }
    }

    func applicationShouldTerminate(_ sender: NSApplication) -> NSApplication.TerminateReply {
        .terminateNow
    }

    func applicationWillTerminate(_ notification: Notification) {
        if let quitEventMonitor {
            NSEvent.removeMonitor(quitEventMonitor)
        }
    }

    func requestTermination(_ application: NSApplication = .shared) {
        guard !terminationPending else { return }
        terminationPending = true
        let finish = { [weak self, weak application] in
            guard let self, let application else { return }
            for sheet in application.windows where sheet.sheetParent != nil {
                sheet.sheetParent?.endSheet(sheet)
            }
            DispatchQueue.main.async {
                application.terminate(nil)
                self.terminationPending = false
            }
        }
        if let prepareForTermination {
            prepareForTermination(finish)
        } else {
            finish()
        }
    }
}

@main
struct BurstFrameDeduplicatorApp: App {
    @NSApplicationDelegateAdaptor(AppLifecycleDelegate.self) private var appDelegate
    @StateObject private var locale = LocaleCatalog()
    @StateObject private var model = AppModel()

    var body: some Scene {
        WindowGroup {
            RootView(model: model)
                .environmentObject(locale)
                .frame(minWidth: 780, minHeight: 580)
                .onAppear {
                    appDelegate.prepareForTermination = model.prepareForTermination
                }
        }
        .defaultSize(width: 1120, height: 760)
        .windowStyle(.titleBar)
        .commands {
            ApplicationCommands(locale: locale, model: model)
        }

        Settings {
            SettingsView(model: model)
                .environmentObject(locale)
        }

        Window(locale.text("aboutTitle"), id: "about") {
            AboutView()
                .environmentObject(locale)
        }
        .windowResizability(.contentSize)
    }
}

private struct ApplicationCommands: Commands {
    @ObservedObject var locale: LocaleCatalog
    @ObservedObject var model: AppModel
    @Environment(\.openWindow) private var openWindow

    var body: some Commands {
        CommandGroup(replacing: .appInfo) {
            Button(locale.text("aboutTitle")) {
                openWindow(id: "about")
            }
        }
        CommandGroup(replacing: .newItem) {
            Button(locale.text("newAppWindow"), action: launchNewInstance)
                .keyboardShortcut("n", modifiers: .command)
            Button(locale.text("newScan")) {
                model.resetForNewScan()
            }
            .keyboardShortcut("n", modifiers: [.command, .shift])
        }
        CommandGroup(after: .help) {
            Button(locale.text("tutorialMenu")) {
                model.showTutorial()
            }
        }
        CommandGroup(replacing: .appTermination) {
            Button(locale.text("quitApp")) {
                terminateApplication()
            }
            .keyboardShortcut("q", modifiers: .command)
        }
    }

    private func terminateApplication() {
        if let delegate = AppLifecycleDelegate.current {
            delegate.requestTermination()
        } else {
            NSApplication.shared.terminate(nil)
        }
    }

    private func launchNewInstance() {
        let appURL = Bundle.main.bundleURL
        guard appURL.pathExtension == "app" else {
            model.resetForNewScan()
            return
        }
        let configuration = NSWorkspace.OpenConfiguration()
        configuration.createsNewApplicationInstance = true
        NSWorkspace.shared.openApplication(at: appURL, configuration: configuration) { _, error in
            if let error {
                DispatchQueue.main.async { model.errorMessage = error.localizedDescription }
            }
        }
    }
}
