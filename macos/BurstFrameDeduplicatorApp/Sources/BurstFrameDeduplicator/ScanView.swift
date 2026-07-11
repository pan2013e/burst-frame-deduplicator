import AppKit
import BurstFrameAppCore
import SwiftUI

struct ScanView: View {
    @EnvironmentObject private var locale: LocaleCatalog
    @ObservedObject var model: AppModel
    @State private var qualityExpanded = false

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
            GroupBox {
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

            GroupBox {
                Grid(alignment: .leading, horizontalSpacing: 18, verticalSpacing: 14) {
                    GridRow {
                        Label(locale.text("acceleration"), systemImage: "bolt")
                        Picker("", selection: $model.options.acceleration) {
                            Text(locale.text("automaticOption")).tag("auto")
                            Text(locale.text("cpuOption")).tag("cpu")
                            Text(locale.text("metalOption")).tag("metal")
                        }
                        .labelsHidden()
                        .frame(maxWidth: 240)
                    }
                    GridRow {
                        Label(locale.text("detector"), systemImage: "viewfinder")
                        Picker("", selection: $model.options.detector) {
                            Text(locale.text("automaticOption")).tag("auto")
                            Text(locale.text("heuristicOption")).tag("heuristic")
                            Text(locale.text("visionOption")).tag("vision")
                            Text(locale.text("offOption")).tag("off")
                        }
                        .labelsHidden()
                        .frame(maxWidth: 240)
                    }
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(4)
            }

            DisclosureGroup(isExpanded: $qualityExpanded) {
                VStack(alignment: .leading, spacing: 12) {
                    Stepper(value: $model.options.previewSize, in: 512...4096, step: 128) {
                        LabeledContent(locale.text("previewSize"), value: "\(model.options.previewSize) px")
                    }
                    Stepper(value: $model.options.refineSize, in: 1024...8192, step: 256) {
                        LabeledContent(locale.text("refineSize"), value: "\(model.options.refineSize) px")
                    }
                }
                .padding(.top, 10)
            } label: {
                Label(locale.text("quality"), systemImage: "slider.horizontal.3")
            }
            .padding(.horizontal, 4)

            HStack {
                Button(action: openRun) {
                    Label(locale.text("openRun"), systemImage: "folder")
                }
                Spacer()
                Button(action: model.startScan) {
                    Label(locale.text("startScan"), systemImage: "sparkles")
                        .frame(minWidth: 112)
                }
                .buttonStyle(.borderedProminent)
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
        .padding(22)
        .background(.background)
        .clipShape(RoundedRectangle(cornerRadius: 8, style: .continuous))
        .overlay {
            RoundedRectangle(cornerRadius: 8, style: .continuous)
                .stroke(Color(nsColor: .separatorColor), lineWidth: 1)
        }
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
