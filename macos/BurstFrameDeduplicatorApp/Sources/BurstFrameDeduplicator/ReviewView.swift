import AppKit
import BurstFrameAppCore
import SwiftUI

struct ReviewView: View {
    @EnvironmentObject private var locale: LocaleCatalog
    @ObservedObject var model: AppModel
    @State private var presentingMove = false
    @State private var confirmingRestore = false

    var body: some View {
        NavigationSplitView {
            sidebar
                .navigationSplitViewColumnWidth(min: 220, ideal: 250, max: 300)
        } detail: {
            stackList
                .navigationTitle(reviewTitle)
                .toolbar {
                    ToolbarItemGroup {
                        if model.fileOperationInProgress {
                            ProgressView()
                                .controlSize(.small)
                                .help(locale.text("working"))
                        }
                        if model.activeMovedCount > 0 {
                            Button {
                                confirmingRestore = true
                            } label: {
                                Label(locale.text("restoreMoved"), systemImage: "arrow.uturn.backward")
                            }
                            .help(locale.text("restoreMoved"))
                            .disabled(model.fileOperationInProgress)
                        }
                        Button {
                            presentingMove = true
                        } label: {
                            Label(
                                model.movableRejectCount == 0
                                    ? locale.text("noRejects")
                                    : locale.text("moveRejects", ["count": model.movableRejectCount]),
                                systemImage: "tray.and.arrow.down"
                            )
                        }
                        .tint(.red)
                        .help(locale.text("moveRejects", ["count": model.movableRejectCount]))
                        .disabled(model.movableRejectCount == 0 || model.fileOperationInProgress)
                        Button(action: model.showRunFolder) {
                            Label(locale.text("showRunFolder"), systemImage: "folder")
                        }
                        .help(locale.text("showRunFolder"))
                        Button(action: model.resetForNewScan) {
                            Label(locale.text("newScan"), systemImage: "plus")
                        }
                        .help(locale.text("newScan"))
                    }
                }
        }
        .confirmationDialog(
            locale.text("restoreConfirmTitle"),
            isPresented: $confirmingRestore,
            titleVisibility: .visible
        ) {
            Button(locale.text("restore"), action: model.restoreMoved)
            Button(locale.text("cancel"), role: .cancel) {}
        } message: {
            Text(locale.text("restoreConfirmMessage"))
        }
        .sheet(isPresented: $presentingMove) {
            MoveRejectsSheet(model: model)
                .environmentObject(locale)
        }
        .sheet(isPresented: Binding(
            get: { model.viewerAssetID != nil },
            set: { if !$0 { model.viewerAssetID = nil } }
        )) {
            NativeImageViewer(model: model)
                .environmentObject(locale)
        }
    }

    private var sidebar: some View {
        VStack(alignment: .leading, spacing: 14) {
            VStack(alignment: .leading, spacing: 3) {
                Text(locale.text("review"))
                    .font(.headline)
                Text(URL(fileURLWithPath: model.payload?.manifest.root ?? "").lastPathComponent)
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .lineLimit(1)
            }

            HStack(spacing: 7) {
                Image(systemName: "magnifyingglass")
                    .foregroundStyle(.secondary)
                TextField(locale.text("findFilename"), text: $model.searchText)
                    .textFieldStyle(.plain)
            }
            .padding(.horizontal, 9)
            .padding(.vertical, 7)
            .background(.quaternary, in: RoundedRectangle(cornerRadius: 7, style: .continuous))

            Picker("", selection: $model.filter) {
                Text(locale.text("allFrames")).tag(ReviewFilter.all)
                Text(locale.text("needsReview")).tag(ReviewFilter.review)
                Text(locale.text("keptFrames")).tag(ReviewFilter.keep)
                Text(locale.text("rejectedFrames")).tag(ReviewFilter.reject)
                Text(locale.text("movedFrames")).tag(ReviewFilter.moved)
                Text(locale.text("multiFrameStacks")).tag(ReviewFilter.multi)
            }
            .labelsHidden()
            .pickerStyle(.menu)

            Divider()

            VStack(spacing: 10) {
                statRow("photo.stack", locale.text("images"), model.payload?.manifest.summary.discoveredAssets ?? 0)
                statRow("square.stack.3d.up", locale.text("bursts"), model.payload?.manifest.summary.bursts ?? 0)
                statRow("rectangle.stack", locale.text("stacks"), model.payload?.manifest.summary.clusters ?? 0)
                statRow("checkmark.circle", locale.text("keep"), model.counts[.keep] ?? 0, color: .green)
                statRow("xmark.circle", locale.text("rejected"), model.counts[.reject] ?? 0, color: .red)
                statRow("tray.full", locale.text("moved"), model.activeMovedCount, color: .blue)
                statRow("questionmark.circle", locale.text("needsReview"), model.counts[.review] ?? 0, color: .orange)
                statRow("pencil", locale.text("manualEdits"), model.manualDecisions.count)
            }

            Spacer()
        }
        .padding(16)
        .frame(maxHeight: .infinity)
        .background(.bar)
    }

    private var stackList: some View {
        Group {
            if model.visibleStacks.isEmpty {
                ContentUnavailableView(
                    locale.text("emptyReview"),
                    systemImage: "photo.on.rectangle.angled"
                )
            } else {
                ScrollView {
                    LazyVStack(spacing: 14) {
                        ForEach(model.visibleStacks) { stack in
                            StackSection(model: model, stack: stack)
                        }
                    }
                    .padding(18)
                }
                .scrollContentBackground(.hidden)
            }
        }
        .background(Color(nsColor: .windowBackgroundColor))
    }

    private var reviewTitle: String {
        let folder = URL(fileURLWithPath: model.payload?.manifest.root ?? "").lastPathComponent
        return locale.text("reviewingFolder", ["folder": folder])
    }

    private func statRow(_ symbol: String, _ label: String, _ value: Int, color: Color = .secondary) -> some View {
        HStack(spacing: 9) {
            Image(systemName: symbol)
                .frame(width: 18)
                .foregroundStyle(color)
            Text(label)
            Spacer()
            Text(value, format: .number)
                .monospacedDigit()
                .foregroundStyle(.secondary)
                .contentTransition(.numericText())
        }
        .font(.callout)
    }
}

private struct StackSection: View {
    @EnvironmentObject private var locale: LocaleCatalog
    @ObservedObject var model: AppModel
    let stack: BurstStack

    private var expanded: Bool { model.expandedStackIDs.contains(stack.id) }
    private var assets: [AssetRecord] { model.stackAssets(stack) }

    var body: some View {
        VStack(spacing: 0) {
            Button {
                model.toggleStack(stack.id)
            } label: {
                HStack(spacing: 12) {
                    Image(systemName: expanded ? "chevron.down" : "chevron.right")
                        .font(.caption.weight(.semibold))
                        .frame(width: 12)
                    VStack(alignment: .leading, spacing: 3) {
                        Text(locale.text("stackTitle", ["burst": stack.burstId, "stack": stack.id]))
                            .font(.headline)
                        Text(locale.text("stackSummary", [
                            "count": stack.assetIds.count,
                            "state": locale.text(expanded ? "expanded" : "collapsed"),
                            "keep": stack.assetIds.compactMap { model.assetsByID[$0] }.filter { model.finalAction(for: $0) == .keep }.count,
                            "confidence": stack.similarityConfidence.formatted(.number.precision(.fractionLength(2))),
                        ]))
                        .font(.caption)
                        .foregroundStyle(.secondary)
                    }
                    Spacer()
                }
                .contentShape(Rectangle())
                .padding(.horizontal, 14)
                .padding(.vertical, 11)
            }
            .buttonStyle(.plain)
            .help(locale.text(expanded ? "collapseStack" : "expandStack"))

            if expanded {
                Divider()
                LazyVGrid(
                    columns: [GridItem(.adaptive(minimum: 220, maximum: 300), spacing: 12)],
                    alignment: .leading,
                    spacing: 12
                ) {
                    let differences = exifDifferences(stack.assetIds.compactMap { model.assetsByID[$0] })
                    ForEach(assets) { asset in
                        FrameCard(model: model, stack: stack, asset: asset, exifDifferences: differences)
                    }
                }
                .padding(12)
                .transition(.opacity.combined(with: .move(edge: .top)))
            }
        }
        .background(Color(nsColor: .controlBackgroundColor).opacity(0.34))
        .clipShape(RoundedRectangle(cornerRadius: 6, style: .continuous))
    }

    private func exifDifferences(_ assets: [AssetRecord]) -> Set<String> {
        var result = Set<String>()
        let values: [(String, (AssetRecord) -> String?)] = [
            ("iso", { asset in asset.metadata.iso.map { String($0) } }),
            ("aperture", { asset in asset.metadata.aperture.map { String($0) } }),
            ("shutter", { $0.metadata.shutter }),
            ("focal", { asset in asset.metadata.focalLengthMm.map { String($0) } }),
            ("equivalent", { asset in asset.metadata.focalLength35mm.map { String($0) } }),
        ]
        for (key, value) in values where Set(assets.compactMap(value)).count > 1 {
            result.insert(key)
        }
        return result
    }
}

private struct FrameCard: View {
    @EnvironmentObject private var locale: LocaleCatalog
    @ObservedObject var model: AppModel
    let stack: BurstStack
    let asset: AssetRecord
    let exifDifferences: Set<String>
    @State private var detailsExpanded = false

    private var decision: FrameDecision { model.finalAction(for: asset) }

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            Button {
                model.viewerAssetID = asset.id
            } label: {
                ZStack(alignment: .topLeading) {
                    CachedImageView(path: thumbnailPath)
                        .frame(maxWidth: .infinity)
                        .aspectRatio(4.0 / 3.0, contentMode: .fit)
                        .clipped()
                        .saturation(decision == .reject && !model.isMoved(asset) ? 0.72 : 1)
                        .opacity(decision == .reject && !model.isMoved(asset) ? 0.82 : 1)
                    Text(statusText)
                        .font(.caption2.weight(.bold))
                        .padding(.horizontal, 6)
                        .padding(.vertical, 4)
                        .foregroundStyle(.white)
                        .background(statusColor.opacity(0.92), in: RoundedRectangle(cornerRadius: 4))
                        .padding(7)
                }
            }
            .buttonStyle(.plain)
            .help(locale.text("openPreview"))

            VStack(alignment: .leading, spacing: 9) {
                HStack(spacing: 6) {
                    TriStateCheckbox(
                        state: decision,
                        accessibilityLabel: locale.text("keep")
                    ) { model.setDecision($0, for: asset) }
                    .fixedSize()
                    Text(locale.text("keep"))
                        .font(.body.weight(.medium))
                    Spacer()
                    Menu {
                        Button(locale.text("keep")) { model.setDecision(.keep, for: asset) }
                        Button(locale.text("rejected")) { model.setDecision(.reject, for: asset) }
                        Button(locale.text("needsReview")) { model.setDecision(.review, for: asset) }
                        if model.manualDecisions[asset.id] != nil {
                            Divider()
                            Button(locale.text("resetSuggestion")) { model.resetDecision(for: asset) }
                        }
                    } label: {
                        Image(systemName: "ellipsis.circle")
                    }
                    .menuStyle(.borderlessButton)
                    .fixedSize()
                }

                Text(asset.representative.relPath)
                    .font(.callout.weight(.medium))
                    .lineLimit(2)
                    .truncationMode(.middle)

                exifView

                Text(reasonText)
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .lineLimit(3)

                DisclosureGroup(isExpanded: $detailsExpanded) {
                    VStack(alignment: .leading, spacing: 5) {
                        detailText(locale.text("rankDetail", ["rank": asset.suggestion.rank, "score": format(asset.suggestion.score, 3)]))
                        detailText(locale.text("sharpnessDetail", ["whole": format(asset.metrics.sharpness, 1), "subject": format(asset.metrics.subjectSharpness, 1)]))
                        detailText(locale.text("similarityDetail", ["distance": format(asset.similarity.nearestDistance, 3), "confidence": format(asset.similarity.duplicateConfidence, 2)]))
                        detailText(locale.text("completenessDetail", ["completeness": format(asset.metrics.completeness, 2), "exposure": format(asset.metrics.exposureScore, 2)]))
                    }
                    .padding(.top, 5)
                } label: {
                    Text(locale.text("why"))
                        .font(.caption)
                }
            }
            .padding(11)
        }
        .background(.background)
        .clipShape(RoundedRectangle(cornerRadius: 6, style: .continuous))
        .overlay {
            RoundedRectangle(cornerRadius: 6, style: .continuous)
                .stroke(borderColor, lineWidth: decision == .keep ? 1.5 : 1)
        }
        .animation(.easeInOut(duration: 0.18), value: decision)
    }

    private var thumbnailPath: String? {
        guard let runDirectory = model.payload?.runDir, let thumb = asset.thumb else { return nil }
        return URL(fileURLWithPath: runDirectory).appendingPathComponent(thumb).path
    }

    private var statusText: String {
        if model.isMoved(asset) { return locale.text("moved") }
        switch decision {
        case .keep: return locale.text("keep")
        case .reject: return locale.text("rejected")
        case .review: return locale.text("needsReview")
        }
    }

    private var statusColor: Color {
        if model.isMoved(asset) { return .blue }
        switch decision {
        case .keep: return .green
        case .reject: return .red
        case .review: return .orange
        }
    }

    private var borderColor: Color {
        if model.isMoved(asset) { return .blue.opacity(0.72) }
        switch decision {
        case .keep: return .green.opacity(0.72)
        case .review: return .orange.opacity(0.68)
        case .reject: return Color(nsColor: .separatorColor)
        }
    }

    private var reasonText: String {
        if asset.error != nil { return locale.text("decodeError") }
        if stack.assetIds.count == 1 { return locale.text("distinctFrame") }
        if asset.suggestion.rank == 1 { return locale.text("bestQuality") }
        if asset.suggestion.action == "review" && asset.similarity.duplicateConfidence < 0.52 {
            return locale.text("uncertainSimilarity")
        }
        if asset.suggestion.action == "review" { return locale.text("qualityTie") }
        return locale.text("duplicate")
    }

    @ViewBuilder
    private var exifView: some View {
        let fields = exifFields
        if fields.isEmpty {
            Text(locale.text("exifUnavailable"))
                .font(.caption2)
                .foregroundStyle(.tertiary)
        } else {
            ViewThatFits(in: .horizontal) {
                HStack(spacing: 4) { chips(fields) }
                VStack(alignment: .leading, spacing: 4) { chips(fields) }
            }
        }
    }

    @ViewBuilder
    private func chips(_ fields: [(String, String)]) -> some View {
        ForEach(fields, id: \.0) { field in
            Text(field.1)
                .font(.caption2.monospacedDigit())
                .padding(.horizontal, 5)
                .padding(.vertical, 3)
                .background(
                    exifDifferences.contains(field.0) ? Color.orange.opacity(0.16) : Color.secondary.opacity(0.08),
                    in: RoundedRectangle(cornerRadius: 4)
                )
        }
    }

    private var exifFields: [(String, String)] {
        var fields: [(String, String)] = []
        if let iso = asset.metadata.iso { fields.append(("iso", locale.text("isoValue", ["value": iso]))) }
        if let aperture = asset.metadata.aperture { fields.append(("aperture", locale.text("apertureValue", ["value": format(aperture, 1)]))) }
        if let shutter = asset.metadata.shutter { fields.append(("shutter", shutter)) }
        if let focal = asset.metadata.focalLengthMm { fields.append(("focal", locale.text("focalValue", ["value": format(focal, 1)]))) }
        if let equivalent = asset.metadata.focalLength35mm { fields.append(("equivalent", locale.text("equivalentFocalValue", ["value": equivalent]))) }
        return fields
    }

    private func detailText(_ value: String) -> some View {
        Text(value).font(.caption2).foregroundStyle(.secondary)
    }

    private func format(_ value: Double, _ digits: Int) -> String {
        value.formatted(.number.precision(.fractionLength(digits)))
    }
}

private struct MoveRejectsSheet: View {
    @EnvironmentObject private var locale: LocaleCatalog
    @Environment(\.dismiss) private var dismiss
    @ObservedObject var model: AppModel
    @State private var destination: URL?
    @State private var confirmingMove = false

    var body: some View {
        VStack(alignment: .leading, spacing: 20) {
            HStack(spacing: 12) {
                Image(systemName: "tray.and.arrow.down.fill")
                    .font(.title2)
                    .foregroundStyle(.red)
                VStack(alignment: .leading, spacing: 2) {
                    Text(locale.text("moveConfirmTitle"))
                        .font(.title3.weight(.semibold))
                    Text(locale.text("moveSelectionSummary", ["count": model.movableRejectCount]))
                        .foregroundStyle(.secondary)
                }
            }

            GroupBox(locale.text("destination")) {
                HStack(spacing: 12) {
                    Image(systemName: "folder")
                        .foregroundStyle(.secondary)
                    Text(destinationLabel)
                        .lineLimit(1)
                        .truncationMode(.middle)
                    Spacer()
                    Button(locale.text("choose"), action: chooseDestination)
                    if destination != nil {
                        Button(locale.text("useRunFolder")) { destination = nil }
                    }
                }
                .padding(.vertical, 4)
            }

            HStack {
                Button(locale.text("cancel"), role: .cancel) { dismiss() }
                Spacer()
                Button(locale.text("move"), role: .destructive) {
                    confirmingMove = true
                }
                .buttonStyle(.borderedProminent)
                .tint(.red)
                .keyboardShortcut(.defaultAction)
            }
        }
        .padding(24)
        .frame(width: 520)
        .onAppear {
            destination = model.defaultMoveDestinationPath.map(URL.init(fileURLWithPath:))
        }
        .confirmationDialog(
            locale.text("moveConfirmTitle"),
            isPresented: $confirmingMove,
            titleVisibility: .visible
        ) {
            Button(locale.text("move"), role: .destructive) {
                model.moveRejects(destination: destination)
                dismiss()
            }
            Button(locale.text("cancel"), role: .cancel) {}
        } message: {
            Text(locale.text("moveConfirmMessage"))
        }
    }

    private var destinationLabel: String {
        guard let destination else { return locale.text("insideRunFolder") }
        return destination.path
    }

    private func chooseDestination() {
        let panel = NSOpenPanel()
        panel.canChooseDirectories = true
        panel.canChooseFiles = false
        panel.canCreateDirectories = true
        panel.allowsMultipleSelection = false
        if panel.runModal() == .OK { destination = panel.url }
    }
}
