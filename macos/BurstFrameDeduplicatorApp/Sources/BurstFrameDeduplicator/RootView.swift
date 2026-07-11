import BurstFrameAppCore
import SwiftUI

struct RootView: View {
    @EnvironmentObject private var locale: LocaleCatalog
    @ObservedObject var model: AppModel

    var body: some View {
        Group {
            if model.phase == .review {
                ReviewView(model: model)
            } else {
                ScanView(model: model)
            }
        }
        .preferredColorScheme(model.appearanceMode.colorScheme)
        .environment(\.locale, Locale(identifier: locale.appleLocaleIdentifier))
        .id(locale.code)
        .alert(
            locale.text("appTitle"),
            isPresented: Binding(
                get: { model.errorMessage != nil },
                set: { if !$0 { model.errorMessage = nil } }
            )
        ) {
            Button(locale.text("close")) { model.errorMessage = nil }
        } message: {
            Text(model.errorMessage ?? "")
        }
        .alert(
            locale.text("appTitle"),
            isPresented: Binding(
                get: { model.notice != nil },
                set: { if !$0 { model.notice = nil } }
            )
        ) {
            Button(locale.text("close")) { model.notice = nil }
        } message: {
            Text(noticeText)
        }
    }

    private var noticeText: String {
        switch model.notice {
        case .moved(let files, let assets, let destination, let failures):
            let summary = locale.text("moveComplete", ["files": files, "assets": assets])
            let location = locale.text("moveDestination", ["destination": destination])
            if failures > 0 {
                return "\(summary)\n\(location)\n\(locale.text("operationFailures", ["count": failures]))"
            }
            return "\(summary)\n\(location)"
        case .restored(let files, let assets, let failures):
            let summary = locale.text("restoreComplete", ["files": files, "assets": assets])
            if failures > 0 {
                return "\(summary)\n\(locale.text("operationFailures", ["count": failures]))"
            }
            return summary
        case .sourceUnavailable(let message):
            return "\(locale.text("sourceUnavailableMove"))\n\n\(message)"
        case .message(let message):
            return message
        case nil:
            return ""
        }
    }
}
