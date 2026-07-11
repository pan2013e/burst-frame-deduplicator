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
            CommandGroup(replacing: .newItem) {
                Button(locale.text("newScan")) {
                    model.resetForNewScan()
                }
                .keyboardShortcut("n", modifiers: .command)
            }
        }
    }
}
