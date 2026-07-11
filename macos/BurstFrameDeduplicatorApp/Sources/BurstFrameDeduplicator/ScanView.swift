import AppKit
import BurstFrameAppCore
import SwiftUI

struct ScanView: View {
    @EnvironmentObject private var locale: LocaleCatalog
    @ObservedObject var model: AppModel
    @StateObject private var runLibrary = RunCacheManager()

    var body: some View {
        ScrollView {
            Group {
                if model.phase == .scanning {
                    scanningContent
                } else {
                    startContent
                }
            }
            .frame(maxWidth: 900, alignment: .leading)
            .padding(.horizontal, 38)
            .padding(.vertical, 32)
            .frame(maxWidth: .infinity)
        }
        .animation(.easeInOut(duration: 0.24), value: model.phase)
        .task { refreshRuns() }
        .onChange(of: model.resultsRootPath) { _, _ in refreshRuns() }
        .id(locale.code)
    }

    private var startContent: some View {
        VStack(alignment: .leading, spacing: 30) {
            HStack(alignment: .top, spacing: 16) {
                Image(systemName: "camera.viewfinder")
                    .font(.system(size: 34, weight: .medium))
                    .foregroundStyle(.tint)
                    .symbolRenderingMode(.hierarchical)
                VStack(alignment: .leading, spacing: 5) {
                    Text(locale.text("appTitle"))
                        .font(.largeTitle.weight(.semibold))
                    Text(locale.text("getStartedSubtitle"))
                        .font(.title3)
                        .foregroundStyle(.secondary)
                }
                Spacer()
                SettingsLink {
                    Label(locale.text("settings"), systemImage: "gearshape")
                }
            }

            HStack(alignment: .top, spacing: 34) {
                quickStart
                    .frame(maxWidth: 340, alignment: .topLeading)
                Divider()
                recentRuns
                    .frame(maxWidth: .infinity, alignment: .topLeading)
            }
        }
    }

    private var quickStart: some View {
        VStack(alignment: .leading, spacing: 14) {
            Label(locale.text("quickStart"), systemImage: "sparkles")
                .font(.headline)

            Button(action: startNewRun) {
                Label(locale.text("newScan"), systemImage: "plus.circle.fill")
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(.vertical, 5)
            }
            .primaryActionStyle()
            .tint(.accentColor)
            .controlSize(.large)
            .keyboardShortcut(.defaultAction)

            Button(action: openRun) {
                Label(locale.text("openRun"), systemImage: "folder")
                    .frame(maxWidth: .infinity, alignment: .leading)
            }
            .controlSize(.large)

            VStack(alignment: .leading, spacing: 5) {
                Text(locale.text("resultsStoredIn"))
                    .font(.caption.weight(.medium))
                    .foregroundStyle(.secondary)
                Text(model.resultsRootPath)
                    .font(.caption)
                    .foregroundStyle(.tertiary)
                    .lineLimit(2)
                    .truncationMode(.middle)
            }
            .padding(.top, 6)
        }
    }

    private var recentRuns: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack {
                Label(locale.text("recentRuns"), systemImage: "clock.arrow.circlepath")
                    .font(.headline)
                Spacer()
                if runLibrary.loading {
                    ProgressView().controlSize(.small)
                }
            }

            if runLibrary.entries.isEmpty, !runLibrary.loading {
                ContentUnavailableView(
                    locale.text("noRecentRuns"),
                    systemImage: "photo.stack",
                    description: Text(locale.text("noRecentRunsDetail"))
                )
                .frame(maxWidth: .infinity, minHeight: 210)
            } else {
                LazyVStack(spacing: 8) {
                    ForEach(runLibrary.entries.prefix(8)) { entry in
                        Button {
                            model.openRun(at: URL(fileURLWithPath: entry.path, isDirectory: true))
                        } label: {
                            HStack(spacing: 11) {
                                Image(systemName: entry.sourceAvailable ? "photo.stack" : "externaldrive.badge.exclamationmark")
                                    .font(.title3)
                                    .frame(width: 26)
                                    .foregroundStyle(entry.sourceAvailable ? Color.accentColor : .orange)
                                VStack(alignment: .leading, spacing: 3) {
                                    Text(URL(fileURLWithPath: entry.sourcePath).lastPathComponent)
                                        .font(.body.weight(.medium))
                                        .lineLimit(1)
                                    HStack(spacing: 7) {
                                        if let createdAt = entry.createdAt {
                                            Text(createdAt, format: .dateTime.year().month(.abbreviated).day().hour().minute())
                                        }
                                        Text(locale.text("photosCount", ["count": entry.assets]))
                                        if !entry.sourceAvailable {
                                            Text(locale.text("sourceOffline"))
                                                .foregroundStyle(.orange)
                                        }
                                    }
                                    .font(.caption)
                                    .foregroundStyle(.secondary)
                                }
                                Spacer()
                                Image(systemName: "chevron.right")
                                    .font(.caption.weight(.semibold))
                                    .foregroundStyle(.tertiary)
                            }
                            .padding(.horizontal, 12)
                            .padding(.vertical, 10)
                            .background(.quaternary.opacity(0.45), in: RoundedRectangle(cornerRadius: 7))
                            .contentShape(Rectangle())
                        }
                        .buttonStyle(.plain)
                        .help(locale.text("continueReview"))
                    }
                }
            }
        }
    }

    private var scanningContent: some View {
        VStack(alignment: .leading, spacing: 24) {
            HStack(spacing: 12) {
                Image(systemName: "camera.viewfinder")
                    .font(.system(size: 28, weight: .medium))
                    .foregroundStyle(.tint)
                VStack(alignment: .leading, spacing: 2) {
                    Text(locale.text("analyzingPhotoFolder"))
                        .font(.title2.weight(.semibold))
                    Text(model.sourceURL?.path ?? "")
                        .font(.callout)
                        .foregroundStyle(.secondary)
                        .lineLimit(1)
                        .truncationMode(.middle)
                }
            }
            progressView
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

    private var stageKeys: [String] {
        ["preparing", "discovering", "analyzing", "grouping", "refining", "ranking", "writing", "exporting", "complete"]
    }

    private var stageLabel: String {
        locale.text(model.progress?.stage ?? "preparing")
    }

    private func stageSymbol(_ stage: String) -> String {
        let current = stageKeys.firstIndex(of: model.progress?.stage ?? "preparing") ?? 0
        let index = stageKeys.firstIndex(of: stage) ?? 0
        if stage == "complete", model.progress?.stage == "complete" { return "checkmark.circle.fill" }
        if index < current { return "checkmark.circle.fill" }
        if index == current { return "circle.inset.filled" }
        return "circle"
    }

    private func stageColor(_ stage: String) -> Color {
        let current = stageKeys.firstIndex(of: model.progress?.stage ?? "preparing") ?? 0
        let index = stageKeys.firstIndex(of: stage) ?? 0
        return index <= current ? .accentColor : .secondary
    }

    private func startNewRun() {
        guard let source = chooseDirectory(for: .photos, locale: locale, startingAt: model.sourceURL) else { return }
        model.startScan(from: source)
    }

    private func openRun() {
        guard let run = chooseDirectory(for: .run, locale: locale) else { return }
        model.openRun(at: run)
    }

    private func refreshRuns() {
        runLibrary.refresh(resultRoots: [model.resultsRootPath])
    }
}
