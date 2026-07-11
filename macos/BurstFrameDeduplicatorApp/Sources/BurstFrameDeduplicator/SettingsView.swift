import AppKit
import BurstFrameAppCore
import SwiftUI

private enum QualityPreset: String, CaseIterable, Identifiable {
    case best
    case balanced
    case fast
    case custom

    var id: String { rawValue }
}

struct SettingsView: View {
    @EnvironmentObject private var locale: LocaleCatalog
    @ObservedObject var model: AppModel
    @StateObject private var cache = RunCacheManager()

    var body: some View {
        TabView {
            generalSettings
                .tabItem { Label(locale.text("general"), systemImage: "gearshape") }
            analysisSettings
                .tabItem { Label(locale.text("analysis"), systemImage: "viewfinder") }
            storageSettings
                .tabItem { Label(locale.text("storage"), systemImage: "internaldrive") }
        }
        .frame(width: 560, height: 430)
    }

    private var generalSettings: some View {
        Form {
            Section(locale.text("appearance")) {
                Picker(locale.text("language"), selection: $locale.code) {
                    ForEach(LocaleCatalog.supportedCodes, id: \.self) { code in
                        Text(locale.languageName(for: code)).tag(code)
                    }
                }
            }

            Section(locale.text("fileOperations")) {
                LabeledContent(locale.text("defaultMoveDestination")) {
                    Text(defaultMoveDestinationName)
                        .foregroundStyle(.secondary)
                        .lineLimit(1)
                        .truncationMode(.middle)
                }
                HStack {
                    Button(locale.text("choose"), action: chooseMoveDestination)
                    if model.defaultMoveDestinationPath != nil {
                        Button(locale.text("useRunFolder")) {
                            model.defaultMoveDestinationPath = nil
                        }
                    }
                }
            }
        }
        .formStyle(.grouped)
        .padding(.top, 8)
    }

    private var analysisSettings: some View {
        Form {
            Section(locale.text("quality")) {
                Picker(locale.text("qualityPreset"), selection: qualityPresetBinding) {
                    Text(locale.text("bestQualityPreset")).tag(QualityPreset.best)
                    Text(locale.text("balancedPreset")).tag(QualityPreset.balanced)
                    Text(locale.text("fastPreset")).tag(QualityPreset.fast)
                    Text(locale.text("customPreset")).tag(QualityPreset.custom)
                }
                Stepper(value: $model.options.previewSize, in: 512...4096, step: 128) {
                    LabeledContent(locale.text("previewSize"), value: "\(model.options.previewSize) px")
                }
                Stepper(value: $model.options.refineSize, in: 1024...8192, step: 256) {
                    LabeledContent(locale.text("refineSize"), value: "\(model.options.refineSize) px")
                }
                Stepper(value: $model.options.refineCandidatesPerCluster, in: 1...8) {
                    LabeledContent(
                        locale.text("refineCandidates"),
                        value: model.options.refineCandidatesPerCluster.formatted()
                    )
                }
                Toggle(locale.text("highResolutionRefinement"), isOn: refinementBinding)
            }

            Section(locale.text("processing")) {
                Picker(locale.text("acceleration"), selection: $model.options.acceleration) {
                    Text(locale.text("automaticOption")).tag("auto")
                    Text(locale.text("cpuOption")).tag("cpu")
                    Text(locale.text("metalOption")).tag("metal")
                }
                Picker(locale.text("detector"), selection: $model.options.detector) {
                    Text(locale.text("automaticOption")).tag("auto")
                    Text(locale.text("heuristicOption")).tag("heuristic")
                    Text(locale.text("visionOption")).tag("vision")
                    Text(locale.text("offOption")).tag("off")
                }
            }
        }
        .formStyle(.grouped)
        .padding(.top, 8)
    }

    private var storageSettings: some View {
        Form {
            Section(locale.text("previousRuns")) {
                LabeledContent(locale.text("runCount"), value: cache.summary.runCount.formatted())
                LabeledContent(locale.text("cacheSize"), value: formattedCacheSize)
                if cache.summary.containsMovedRejects {
                    Label(locale.text("cacheContainsMoved"), systemImage: "arrow.uturn.backward.circle")
                        .foregroundStyle(.orange)
                }
                HStack {
                    Button(locale.text("recalculate")) {
                        cache.refresh(excluding: model.payload?.runDir)
                    }
                    Spacer()
                    Button(locale.text("removePreviousRuns"), role: .destructive) {
                        confirmingCacheRemoval = true
                    }
                    .disabled(cache.summary.runCount == 0 || cache.loading)
                }
            }
        }
        .formStyle(.grouped)
        .padding(.top, 8)
        .overlay {
            if cache.loading { ProgressView().controlSize(.small) }
        }
        .task { cache.refresh(excluding: model.payload?.runDir) }
        .confirmationDialog(
            locale.text("removeCacheTitle"),
            isPresented: $confirmingCacheRemoval,
            titleVisibility: .visible
        ) {
            Button(locale.text("removePreviousRuns"), role: .destructive) {
                cache.removePreviousRuns(excluding: model.payload?.runDir)
            }
            Button(locale.text("cancel"), role: .cancel) {}
        } message: {
            Text(locale.text(cache.summary.containsMovedRejects ? "removeCacheMovedMessage" : "removeCacheMessage"))
        }
        .alert(
            locale.text("storage"),
            isPresented: Binding(
                get: { cache.errorMessage != nil },
                set: { if !$0 { cache.errorMessage = nil } }
            )
        ) {
            Button(locale.text("close")) { cache.errorMessage = nil }
        } message: {
            Text(cache.errorMessage ?? "")
        }
    }

    @State private var confirmingCacheRemoval = false

    private var qualityPresetBinding: Binding<QualityPreset> {
        Binding(
            get: { inferredPreset },
            set: { applyPreset($0) }
        )
    }

    private var refinementBinding: Binding<Bool> {
        Binding(
            get: { !model.options.disableRefinement },
            set: { model.options.disableRefinement = !$0 }
        )
    }

    private var inferredPreset: QualityPreset {
        let options = model.options
        if options.previewSize == 2048 && options.refineSize == 4096
            && options.refineCandidatesPerCluster == 4
            && options.maxDuplicateDistance == 0.18
            && options.minDuplicateConfidence == 0.60
            && options.detector == "vision"
            && !options.disableRefinement
        { return .best }
        if options.previewSize == 1280 && options.refineSize == 2048
            && options.refineCandidatesPerCluster == 2 && !options.disableRefinement
        { return .balanced }
        if options.previewSize == 960 && options.refineSize == 1536
            && options.refineCandidatesPerCluster == 1 && !options.disableRefinement
        { return .fast }
        return .custom
    }

    private func applyPreset(_ preset: QualityPreset) {
        switch preset {
        case .best:
            model.options.previewSize = 2048
            model.options.refineSize = 4096
            model.options.refineCandidatesPerCluster = 4
            model.options.maxDuplicateDistance = 0.18
            model.options.minDuplicateConfidence = 0.60
            model.options.detector = "vision"
            model.options.disableRefinement = false
        case .balanced:
            model.options.previewSize = 1280
            model.options.refineSize = 2048
            model.options.refineCandidatesPerCluster = 2
            model.options.maxDuplicateDistance = 0.20
            model.options.minDuplicateConfidence = 0.52
            model.options.detector = "auto"
            model.options.disableRefinement = false
        case .fast:
            model.options.previewSize = 960
            model.options.refineSize = 1536
            model.options.refineCandidatesPerCluster = 1
            model.options.maxDuplicateDistance = 0.20
            model.options.minDuplicateConfidence = 0.52
            model.options.detector = "heuristic"
            model.options.disableRefinement = false
        case .custom:
            break
        }
    }

    private var defaultMoveDestinationName: String {
        guard let path = model.defaultMoveDestinationPath else { return locale.text("insideRunFolder") }
        return URL(fileURLWithPath: path).lastPathComponent
    }

    private var formattedCacheSize: String {
        ByteCountFormatter.string(fromByteCount: Int64(cache.summary.bytes), countStyle: .file)
    }

    private func chooseMoveDestination() {
        let panel = NSOpenPanel()
        panel.canChooseDirectories = true
        panel.canChooseFiles = false
        panel.canCreateDirectories = true
        panel.allowsMultipleSelection = false
        if panel.runModal() == .OK {
            model.defaultMoveDestinationPath = panel.url?.path
        }
    }
}
