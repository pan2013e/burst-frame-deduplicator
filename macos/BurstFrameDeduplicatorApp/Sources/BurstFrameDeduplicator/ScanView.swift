import AppKit
import BurstFrameAppCore
import SwiftUI

struct ScanView: View {
    @EnvironmentObject private var locale: LocaleCatalog
    @ObservedObject var model: AppModel

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 22) {
                HStack(spacing: 12) {
                    Image(systemName: "camera.viewfinder")
                        .font(.system(size: 28, weight: .medium))
                        .foregroundStyle(.tint)
                    Text(locale.text("appTitle"))
                        .font(.title2.weight(.semibold))
                }

                if model.phase == .scanning {
                    progressView
                        .transition(.opacity.combined(with: .move(edge: .bottom)))
                } else {
                    configurationForm
                        .transition(.opacity)
                }
            }
            .frame(maxWidth: 760, alignment: .leading)
            .padding(.horizontal, 32)
            .padding(.vertical, 28)
            .frame(maxWidth: .infinity)
        }
        .animation(.easeInOut(duration: 0.24), value: model.phase)
    }

    private var configurationForm: some View {
        VStack(alignment: .leading, spacing: 18) {
            GroupBox(locale.text("folders")) {
                VStack(spacing: 0) {
                    folderRow(
                        title: locale.text("photoFolder"),
                        value: model.sourceURL?.lastPathComponent,
                        icon: "photo.on.rectangle.angled",
                        action: chooseSource
                    )
                    Divider().padding(.leading, 36)
                    folderRow(
                        title: locale.text("runFolder"),
                        value: model.outputURL?.lastPathComponent ?? locale.text("automatic"),
                        icon: "folder.badge.gearshape",
                        action: chooseOutput
                    )
                }
            }

            HStack {
                Button(action: openRun) {
                    Label(locale.text("openRun"), systemImage: "folder")
                }
                SettingsLink {
                    Label(locale.text("settings"), systemImage: "gearshape")
                }
                Spacer()
                Button(action: model.startScan) {
                    Label(locale.text("startScan"), systemImage: "sparkles")
                        .frame(minWidth: 112)
                }
                .primaryActionStyle()
                .controlSize(.large)
                .disabled(model.sourceURL == nil)
                .keyboardShortcut(.defaultAction)
            }
        }
    }

    private var progressView: some View {
        VStack(alignment: .leading, spacing: 20) {
            let fraction = model.progress?.overallFraction ?? 0
            ProgressView(value: fraction)
                .progressViewStyle(.linear)
                .animation(.smooth(duration: 0.2), value: fraction)

            HStack(alignment: .firstTextBaseline) {
                Text(stageLabel)
                    .font(.headline)
                Spacer()
                Text(fraction, format: .percent.precision(.fractionLength(0)))
                    .font(.title3.monospacedDigit().weight(.semibold))
                    .contentTransition(.numericText())
            }

            if let progress = model.progress {
                HStack {
                    if let total = progress.total {
                        Text(locale.text("progressCount", ["current": progress.current, "total": total]))
                            .monospacedDigit()
                    }
                    Spacer()
                    if let detail = progress.detail {
                        Text(URL(fileURLWithPath: detail).lastPathComponent)
                            .lineLimit(1)
                            .truncationMode(.middle)
                            .foregroundStyle(.secondary)
                    }
                }
                .font(.callout)
            }

            VStack(alignment: .leading, spacing: 9) {
                ForEach(stageKeys, id: \.self) { stage in
                    HStack(spacing: 9) {
                        Image(systemName: stageSymbol(stage))
                            .foregroundStyle(stageColor(stage))
                            .contentTransition(.symbolEffect(.replace))
                        Text(locale.text(stage))
                            .foregroundStyle(stage == model.progress?.stage ? .primary : .secondary)
                    }
                }
            }
            .font(.callout)
        }
        .padding(.vertical, 8)
    }

    private func folderRow(title: String, value: String?, icon: String, action: @escaping () -> Void) -> some View {
        HStack(spacing: 12) {
            Image(systemName: icon)
                .frame(width: 24)
                .foregroundStyle(.secondary)
            VStack(alignment: .leading, spacing: 2) {
                Text(title).font(.body.weight(.medium))
                Text(value ?? locale.text("choose"))
                    .font(.callout)
                    .foregroundStyle(.secondary)
                    .lineLimit(1)
            }
            Spacer()
            Button(locale.text("choose"), action: action)
        }
        .padding(12)
    }

    private var stageKeys: [String] {
        ["preparing", "discovering", "analyzing", "grouping", "refining", "ranking", "writing", "exporting", "complete"]
    }

    private var stageLabel: String {
        locale.text(model.progress?.stage ?? "preparing")
    }

    private func stageSymbol(_ stage: String) -> String {
        let current = stageKeys.firstIndex(of: model.progress?.stage ?? "preparing") ?? 0
        let index = stageKeys.firstIndex(of: stage) ?? 0
        if stage == "complete" && model.progress?.stage == "complete" { return "checkmark.circle.fill" }
        if index < current { return "checkmark.circle.fill" }
        if index == current { return "circle.inset.filled" }
        return "circle"
    }

    private func stageColor(_ stage: String) -> Color {
        let current = stageKeys.firstIndex(of: model.progress?.stage ?? "preparing") ?? 0
        let index = stageKeys.firstIndex(of: stage) ?? 0
        return index <= current ? .accentColor : .secondary
    }

    private func chooseSource() {
        if let url = chooseDirectory() { model.sourceURL = url }
    }

    private func chooseOutput() {
        if let url = chooseDirectory() { model.outputURL = url }
    }

    private func openRun() {
        guard let url = chooseDirectory() else { return }
        model.openRun(at: url)
    }

    private func chooseDirectory() -> URL? {
        let panel = NSOpenPanel()
        panel.canChooseDirectories = true
        panel.canChooseFiles = false
        panel.allowsMultipleSelection = false
        panel.canCreateDirectories = true
        return panel.runModal() == .OK ? panel.url : nil
    }
}
