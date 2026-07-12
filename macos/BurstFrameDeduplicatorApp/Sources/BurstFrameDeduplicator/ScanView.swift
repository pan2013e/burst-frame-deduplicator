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
                } else if model.phase == .loading {
                    loadingContent
                } else {
                    startContent
                }
            }
            .frame(maxWidth: 980, alignment: .leading)
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
        VStack(alignment: .leading, spacing: 26) {
            HStack(alignment: .top, spacing: 16) {
                Image(nsImage: NSApplication.shared.applicationIconImage)
                    .resizable()
                    .scaledToFit()
                    .frame(width: 64, height: 64)
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

            quickStart
            Divider()
            recentRuns
                .frame(maxWidth: .infinity, alignment: .topLeading)

            HStack(spacing: 9) {
                Image(systemName: "folder.badge.gearshape")
                    .foregroundStyle(.secondary)
                Text(locale.text("resultsStoredIn"))
                    .foregroundStyle(.secondary)
                Text(model.resultsRootPath)
                    .foregroundStyle(.tertiary)
                    .lineLimit(1)
                    .truncationMode(.middle)
                Spacer()
            }
            .font(.caption)
        }
    }

    private var quickStart: some View {
        VStack(alignment: .leading, spacing: 12) {
            Label(locale.text("quickStart"), systemImage: "sparkles")
                .font(.headline)

            HStack(spacing: 12) {
                Button(action: startNewRun) {
                    Label(locale.text("newScan"), systemImage: "plus.circle.fill")
                        .frame(maxWidth: .infinity, minHeight: 28, alignment: .leading)
                        .padding(.vertical, 4)
                }
                .primaryActionStyle()
                .tint(.accentColor)
                .controlSize(.large)
                .keyboardShortcut(.defaultAction)

                Button(action: openRun) {
                    Label(locale.text("openRun"), systemImage: "folder")
                        .frame(maxWidth: .infinity, minHeight: 28, alignment: .leading)
                        .padding(.vertical, 4)
                }
                .buttonStyle(.bordered)
                .controlSize(.large)
            }
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
                LazyVGrid(
                    columns: [GridItem(.adaptive(minimum: 330, maximum: 470), spacing: 10)],
                    alignment: .leading,
                    spacing: 10
                ) {
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
                            .padding(.vertical, 11)
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

    private var loadingContent: some View {
        VStack(spacing: 22) {
            ZStack {
                RoundedRectangle(cornerRadius: 8)
                    .fill(.quaternary.opacity(0.45))
                    .frame(width: 76, height: 76)
                Image(systemName: "photo.stack")
                    .font(.system(size: 30, weight: .medium))
                    .foregroundStyle(.tint)
                    .symbolRenderingMode(.hierarchical)
            }

            VStack(spacing: 6) {
                Text(locale.text("openingRun"))
                    .font(.title2.weight(.semibold))
                Text(model.loadingRunName)
                    .foregroundStyle(.secondary)
                    .lineLimit(1)
                    .truncationMode(.middle)
            }

            VStack(spacing: 10) {
                if let update = model.loadingProgress {
                    ProgressView(value: update.overallFraction)
                        .animation(.smooth(duration: 0.2), value: update.overallFraction)
                    HStack {
                        ProgressView()
                            .controlSize(.small)
                        Text(locale.text(update.stage))
                        Spacer()
                        Text(update.overallFraction, format: .percent.precision(.fractionLength(0)))
                            .monospacedDigit()
                            .contentTransition(.numericText())
                    }
                    .font(.callout)
                } else {
                    ProgressView()
                        .controlSize(.large)
                    Text(locale.text("reading_manifest"))
                        .font(.callout)
                        .foregroundStyle(.secondary)
                }
            }
            .frame(maxWidth: 520)
        }
        .frame(maxWidth: .infinity, minHeight: 420)
        .transition(.opacity.combined(with: .scale(scale: 0.98)))
    }

    private var progressView: some View {
        VStack(alignment: .leading, spacing: 20) {
            let fraction = model.progress?.overallFraction ?? 0
            ProgressView(value: fraction)
                .progressViewStyle(.linear)
                .animation(.smooth(duration: 0.2), value: fraction)

            HStack(alignment: .firstTextBaseline) {
                Text(model.scanCancellationRequested ? locale.text("cancellingScan") : stageLabel)
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

            HStack {
                Spacer()
                Button(role: .cancel, action: model.cancelScan) {
                    Label(
                        locale.text(model.scanCancellationRequested ? "cancellingScan" : "cancelScan"),
                        systemImage: "xmark.circle"
                    )
                }
                .disabled(model.scanCancellationRequested)
            }
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
