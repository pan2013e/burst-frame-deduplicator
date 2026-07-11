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
        .toolbar {
            ToolbarItem(placement: .automatic) {
                Picker(locale.text("language"), selection: $locale.code) {
                    ForEach(LocaleCatalog.supportedCodes, id: \.self) { code in
                        Text(locale.languageName(for: code)).tag(code)
                    }
                }
                .pickerStyle(.segmented)
                .frame(width: 190)
            }
        }
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
                get: { model.noticeMessage != nil },
                set: { if !$0 { model.noticeMessage = nil } }
            )
        ) {
            Button(locale.text("close")) { model.noticeMessage = nil }
        } message: {
            Text(moveNotice)
        }
    }

    private var moveNotice: String {
        let parts = (model.noticeMessage ?? "").split(separator: "|")
        guard parts.count == 2 else { return model.noticeMessage ?? "" }
        return locale.text("moveComplete", ["files": parts[0], "assets": parts[1]])
    }
}
