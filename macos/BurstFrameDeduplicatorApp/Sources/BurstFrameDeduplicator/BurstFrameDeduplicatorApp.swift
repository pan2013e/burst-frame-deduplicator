import AppKit
import BurstFrameAppCore
import SwiftUI

@main
struct BurstFrameDeduplicatorApp: App {
    @StateObject private var locale = LocaleCatalog()
    @StateObject private var model = AppModel()

    var body: some Scene {
        WindowGroup {
            RootView(model: model)
                .environmentObject(locale)
                .frame(minWidth: 780, minHeight: 580)
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
                .preferredColorScheme(model.appearanceMode.colorScheme)
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
