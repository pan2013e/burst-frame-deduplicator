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
    @State private var confirmingCacheRemoval = false

    var body: some View {
        TabView {
            generalSettings
                .tabItem { Label(locale.text("general"), systemImage: "gearshape") }
            analysisSettings
                .tabItem { Label(locale.text("analysis"), systemImage: "viewfinder") }
            storageSettings
                .tabItem { Label(locale.text("storage"), systemImage: "internaldrive") }
        }
        .frame(width: 650, height: 560)
        .preferredColorScheme(model.appearanceMode.colorScheme)
        .environment(\.locale, Locale(identifier: locale.appleLocaleIdentifier))
        .id(locale.code)
    }

    private var generalSettings: some View {
        Form {
            Section(locale.text("appearance")) {
                Picker(locale.text("language"), selection: $locale.code) {
                    ForEach(LocaleCatalog.supportedCodes, id: \.self) { code in
                        Text(locale.languageName(for: code)).tag(code)
                    }
                }
                Picker(locale.text("colorAppearance"), selection: $model.appearanceMode) {
                    Text(locale.text("followSystem")).tag(AppearanceMode.system)
                    Text(locale.text("lightMode")).tag(AppearanceMode.light)
                    Text(locale.text("darkMode")).tag(AppearanceMode.dark)
                }
            }

            Section(locale.text("resultStorage")) {
                LabeledContent(locale.text("resultDirectory")) {
                    Text(URL(fileURLWithPath: model.resultsRootPath).lastPathComponent)
                        .foregroundStyle(.secondary)
                        .lineLimit(1)
                        .truncationMode(.middle)
                }
                Text(model.resultsRootPath)
                    .font(.caption)
                    .foregroundStyle(.tertiary)
                    .lineLimit(2)
                    .truncationMode(.middle)
                HStack {
                    Button(locale.text("choose"), action: chooseResultsDirectory)
                    if model.resultsRootPath != RunCacheManager.defaultRunsDirectory.path {
                        Button(locale.text("restoreDefault"), action: model.resetResultsRoot)
                    }
                    Spacer()
                }
                if model.relocationInProgress {
                    relocationProgress
                } else if model.payload != nil {
                    Label(locale.text("existingRunMovesWithDirectory"), systemImage: "arrow.right.arrow.left")
                        .font(.caption)
                        .foregroundStyle(.secondary)
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
        .disabled(model.relocationInProgress)
        .overlay(alignment: .bottom) {
            if model.relocationInProgress {
                relocationProgress
                    .padding(12)
                    .background(.regularMaterial, in: RoundedRectangle(cornerRadius: 8))
                    .padding()
                    .allowsHitTesting(false)
            }
        }
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
                NumericStepperRow(
                    label: locale.text("previewSize"),
                    valueText: "\(model.options.previewSize) px",
                    value: $model.options.previewSize,
                    range: 512 ... 4096,
                    step: 128
                )
                NumericStepperRow(
                    label: locale.text("refineSize"),
                    valueText: "\(model.options.refineSize) px",
                    value: $model.options.refineSize,
                    range: 1024 ... 8192,
                    step: 256
                )
                NumericStepperRow(
                    label: locale.text("refineCandidates"),
                    valueText: model.options.refineCandidatesPerCluster.formatted(),
                    value: $model.options.refineCandidatesPerCluster,
                    range: 1 ... 8,
                    step: 1
                )
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

            Section(locale.text("deviceAssessment")) {
                assessmentRow(
                    title: locale.text("deviceCapability"),
                    value: SystemCapability.current.capabilityScore,
                    label: capabilityLabel,
                    colors: [.red, .orange, .yellow, .green]
                )
                assessmentRow(
                    title: locale.text("estimatedSystemLoad"),
                    value: SystemCapability.current.estimatedLoad(for: model.options),
                    label: loadLabel,
                    colors: [.green, .yellow, .orange, .red]
                )
                Text(locale.text("deviceSummary", [
                    "cpu": SystemCapability.current.logicalCPUCount,
                    "memory": formattedMemory,
                    "gpu": SystemCapability.current.gpuName,
                ]))
                .font(.caption)
                .foregroundStyle(.secondary)
            }
        }
        .formStyle(.grouped)
        .padding(.top, 8)
        .disabled(model.relocationInProgress)
    }

    private var storageSettings: some View {
        Form {
            Section {
                LabeledContent(locale.text("runCount"), value: cache.summary.runCount.formatted())
                LabeledContent(locale.text("cacheSize"), value: formattedCacheSize)
                Text(locale.text("cacheScopeDetail"))
                    .font(.caption)
                    .foregroundStyle(.secondary)
            } header: {
                Text(locale.text("previousRuns"))
            }

            Section {
                if cache.entries.isEmpty, !cache.loading {
                    Text(locale.text("noPreviousRuns"))
                        .foregroundStyle(.secondary)
                } else {
                    Toggle(isOn: allSelectedBinding) {
                        Text(locale.text("selectAllRuns"))
                            .fontWeight(.medium)
                    }
                    .toggleStyle(.checkbox)

                    ForEach(cache.entries) { entry in
                        Toggle(isOn: selectionBinding(entry.path)) {
                            runSelectionLabel(entry)
                        }
                        .toggleStyle(.checkbox)
                        .disabled(entry.path == model.payload?.runDir)
                    }
                }
            } header: {
                Text(locale.text("chooseRunsToRemove"))
            }

            Section {
                if cache.selectedSummary.containsMovedRejects {
                    Label(locale.text("cacheContainsMoved"), systemImage: "arrow.uturn.backward.circle")
                        .foregroundStyle(.orange)
                }
                HStack {
                    Button(locale.text("recalculate"), action: refreshCache)
                    Spacer()
                    Button(locale.text("removeSelectedRuns"), role: .destructive) {
                        confirmingCacheRemoval = true
                    }
                    .disabled(cache.selectedSummary.runCount == 0 || cache.loading)
                }
            }
        }
        .formStyle(.grouped)
        .padding(.top, 8)
        .overlay {
            if cache.loading { ProgressView().controlSize(.small) }
        }
        .task { refreshCache() }
        .onChange(of: model.resultsRootPath) { _, _ in refreshCache() }
        .onChange(of: model.payload?.runDir) { _, _ in refreshCache() }
        .confirmationDialog(
            locale.text("removeCacheTitle"),
            isPresented: $confirmingCacheRemoval,
            titleVisibility: .visible
        ) {
            Button(locale.text("removeSelectedRuns"), role: .destructive) {
                cache.removeSelected()
            }
            Button(locale.text("cancel"), role: .cancel) {}
        } message: {
            Text(locale.text(
                cache.selectedSummary.containsMovedRejects
                    ? "removeCacheMovedMessage"
                    : "removeCacheMessage",
                ["count": cache.selectedSummary.runCount]
            ))
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
        .disabled(model.relocationInProgress)
    }

    private var relocationProgress: some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack {
                Label(locale.text("movingRunFolder"), systemImage: "folder.badge.gearshape")
                Spacer()
                Text(model.relocationProgress?.overallFraction ?? 0, format: .percent.precision(.fractionLength(0)))
                    .monospacedDigit()
            }
            ProgressView(value: model.relocationProgress?.overallFraction ?? 0)
            if let detail = model.relocationProgress?.detail {
                Text(detail)
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .lineLimit(1)
                    .truncationMode(.middle)
            }
        }
    }

    private func assessmentRow(
        title: String,
        value: Double,
        label: String,
        colors: [Color]
    ) -> some View {
        VStack(alignment: .leading, spacing: 7) {
            HStack {
                Text(title)
                Spacer()
                Text(label)
                    .foregroundStyle(.secondary)
            }
            ContinuousLevelBar(value: value, colors: colors)
        }
        .padding(.vertical, 2)
    }

    private var qualityPresetBinding: Binding<QualityPreset> {
        Binding(get: { inferredPreset }, set: { applyPreset($0) })
    }

    private var refinementBinding: Binding<Bool> {
        Binding(
            get: { !model.options.disableRefinement },
            set: { model.options.disableRefinement = !$0 }
        )
    }

    private var allSelectedBinding: Binding<Bool> {
        let selectable = Set(cache.entries.lazy.map(\.path).filter { $0 != model.payload?.runDir })
        return Binding(
            get: { !selectable.isEmpty && selectable.isSubset(of: cache.selectedPaths) },
            set: { cache.setAllSelected($0) }
        )
    }

    private func selectionBinding(_ path: String) -> Binding<Bool> {
        Binding(
            get: { cache.selectedPaths.contains(path) },
            set: { selected in
                if selected != cache.selectedPaths.contains(path) { cache.toggleSelection(path) }
            }
        )
    }

    private var inferredPreset: QualityPreset {
        let options = model.options
        if options.previewSize == 2048, options.refineSize == 4096,
           options.refineCandidatesPerCluster == 4,
           options.maxDuplicateDistance == 0.18,
           options.minDuplicateConfidence == 0.60,
           options.detector == "vision",
           !options.disableRefinement
        { return .best }
        if options.previewSize == 1280, options.refineSize == 2048,
           options.refineCandidatesPerCluster == 2, !options.disableRefinement
        { return .balanced }
        if options.previewSize == 960, options.refineSize == 1536,
           options.refineCandidatesPerCluster == 1, !options.disableRefinement
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
        ByteCountFormatter.string(fromByteCount: Int64(clamping: cache.summary.bytes), countStyle: .file)
    }

    private var formattedMemory: String {
        ByteCountFormatter.string(
            fromByteCount: Int64(clamping: SystemCapability.current.memoryBytes),
            countStyle: .memory
        )
    }

    private var capabilityLabel: String {
        levelLabel(SystemCapability.current.capabilityScore, lowKey: "entryLevel", middleKey: "capable", highKey: "highPerformance")
    }

    private var loadLabel: String {
        levelLabel(SystemCapability.current.estimatedLoad(for: model.options), lowKey: "lightLoad", middleKey: "moderateLoad", highKey: "heavyLoad")
    }

    private func levelLabel(_ value: Double, lowKey: String, middleKey: String, highKey: String) -> String {
        if value < 0.38 { return locale.text(lowKey) }
        if value < 0.72 { return locale.text(middleKey) }
        return locale.text(highKey)
    }

    private func runSelectionLabel(_ entry: RunLibraryEntry) -> some View {
        HStack {
            VStack(alignment: .leading, spacing: 2) {
                HStack(spacing: 6) {
                    Text(entry.name).lineLimit(1)
                    if entry.path == model.payload?.runDir {
                        Text(locale.text("currentRun"))
                            .font(.caption2)
                            .foregroundStyle(.secondary)
                    }
                    if entry.containsMovedRejects {
                        Image(systemName: "arrow.uturn.backward.circle.fill")
                            .foregroundStyle(.orange)
                    }
                }
                Text(URL(fileURLWithPath: entry.sourcePath).lastPathComponent)
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
            Spacer()
            Text(ByteCountFormatter.string(fromByteCount: Int64(clamping: entry.bytes), countStyle: .file))
                .font(.caption.monospacedDigit())
                .foregroundStyle(.secondary)
        }
    }

    private func chooseResultsDirectory() {
        let current = URL(fileURLWithPath: model.resultsRootPath, isDirectory: true)
        guard let selected = chooseDirectory(for: .results, locale: locale, startingAt: current) else { return }
        model.changeResultsRoot(to: selected)
    }

    private func chooseMoveDestination() {
        let current = model.defaultMoveDestinationPath.map { URL(fileURLWithPath: $0, isDirectory: true) }
        if let selected = chooseDirectory(for: .moveDestination, locale: locale, startingAt: current) {
            model.defaultMoveDestinationPath = selected.path
        }
    }

    private func refreshCache() {
        cache.refresh(resultRoots: [model.resultsRootPath], excluding: model.payload?.runDir)
    }
}

private struct NumericStepperRow<Value>: View where Value: Strideable, Value.Stride: BinaryInteger {
    let label: String
    let valueText: String
    @Binding var value: Value
    let range: ClosedRange<Value>
    let step: Value.Stride

    var body: some View {
        HStack {
            Text(label)
            Spacer()
            Text(valueText)
                .monospacedDigit()
                .foregroundStyle(.secondary)
                .frame(width: 82, alignment: .trailing)
            Stepper("", value: $value, in: range, step: step)
                .labelsHidden()
                .fixedSize()
        }
    }
}
