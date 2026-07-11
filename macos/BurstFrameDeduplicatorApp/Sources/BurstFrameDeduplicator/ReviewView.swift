import BurstFrameAppCore
import SwiftUI

struct ReviewView: View {
    @EnvironmentObject private var locale: LocaleCatalog
    @ObservedObject var model: AppModel
    @State private var confirmingMove = false

    var body: some View {
        NavigationSplitView {
            sidebar
                .navigationSplitViewColumnWidth(min: 220, ideal: 250, max: 300)
        } detail: {
            stackList
                .navigationTitle(reviewTitle)
                .toolbar {
                    ToolbarItemGroup {
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
            locale.text("moveConfirmTitle"),
            isPresented: $confirmingMove,
            titleVisibility: .visible
        ) {
            Button(locale.text("move"), role: .destructive, action: model.moveRejects)
            Button(locale.text("cancel"), role: .cancel) {}
        } message: {
            Text(locale.text("moveConfirmMessage"))
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
                statRow("questionmark.circle", locale.text("needsReview"), model.counts[.review] ?? 0, color: .orange)
                statRow("pencil", locale.text("manualEdits"), model.manualDecisions.count)
            }

            Spacer()

            Button {
                confirmingMove = true
            } label: {
                Label(
                    rejectCount == 0
                        ? locale.text("noRejects")
                        : locale.text("moveRejects", ["count": rejectCount]),
                    systemImage: "tray.and.arrow.down"
                )
                .frame(maxWidth: .infinity)
            }
            .buttonStyle(.borderedProminent)
            .tint(.red)
            .disabled(rejectCount == 0)
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

    private var rejectCount: Int { model.counts[.reject] ?? 0 }

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
        .background(.background)
        .clipShape(RoundedRectangle(cornerRadius: 7, style: .continuous))
        .overlay {
            RoundedRectangle(cornerRadius: 7, style: .continuous)
                .stroke(Color(nsColor: .separatorColor), lineWidth: 1)
        }
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
        .opacity(decision == .reject ? 0.76 : 1)
        .animation(.easeInOut(duration: 0.18), value: decision)
    }

    private var thumbnailPath: String? {
        guard let runDirectory = model.payload?.runDir, let thumb = asset.thumb else { return nil }
        return URL(fileURLWithPath: runDirectory).appendingPathComponent(thumb).path
    }

    private var statusText: String {
        switch decision {
        case .keep: locale.text("keep")
        case .reject: locale.text("rejected")
        case .review: locale.text("needsReview")
        }
    }

    private var statusColor: Color {
        switch decision {
        case .keep: .green
        case .reject: .red
        case .review: .orange
        }
    }

    private var borderColor: Color {
        switch decision {
        case .keep: .green.opacity(0.72)
        case .review: .orange.opacity(0.68)
        case .reject: Color(nsColor: .separatorColor)
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
