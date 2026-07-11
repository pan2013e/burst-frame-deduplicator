import AppKit
import BurstFrameAppCore
import Foundation
import SwiftUI

enum AppPhase: Equatable {
    case setup
    case scanning
    case review
}

enum FrameDecision: String, CaseIterable {
    case keep
    case reject
    case review
}

enum ReviewFilter: String, CaseIterable, Identifiable {
    case all
    case review
    case keep
    case reject
    case moved
    case multi

    var id: String { rawValue }
}

enum AppNotice: Equatable {
    case moved(files: Int, assets: Int, destination: String, failures: Int)
    case restored(files: Int, assets: Int, failures: Int)
    case sourceUnavailable(String)
    case message(String)
}

enum AppearanceMode: String, CaseIterable, Identifiable {
    case system
    case light
    case dark

    var id: String { rawValue }

    var colorScheme: ColorScheme? {
        switch self {
        case .system: nil
        case .light: .light
        case .dark: .dark
        }
    }
}

@MainActor
final class AppModel: ObservableObject {
    @Published var phase: AppPhase = .setup
    @Published var sourceURL: URL?
    @Published var outputURL: URL?
    @Published private(set) var resultsRootPath: String
    @Published var appearanceMode: AppearanceMode {
        didSet { UserDefaults.standard.set(appearanceMode.rawValue, forKey: "appearanceMode") }
    }
    @Published var options = ScanOptions() {
        didSet { persistOptions() }
    }
    @Published var defaultMoveDestinationPath: String? {
        didSet {
            UserDefaults.standard.set(defaultMoveDestinationPath, forKey: "defaultMoveDestinationPath")
        }
    }
    @Published var progress: ProgressUpdate?
    @Published var payload: ReviewPayload? {
        didSet {
            assetIndex = Dictionary(uniqueKeysWithValues: (payload?.manifest.assets ?? []).map { ($0.id, $0) })
        }
    }
    @Published var manualDecisions: [String: String] = [:]
    @Published var expandedStackIDs: Set<Int> = []
    @Published var searchText = ""
    @Published var filter: ReviewFilter = .all
    @Published var viewerAssetID: String?
    @Published var errorMessage: String?
    @Published var notice: AppNotice?
    @Published var fileOperationInProgress = false
    @Published var relocationInProgress = false
    @Published var relocationProgress: ProgressUpdate?

    private let bridge: RustBridge
    private let decisionQueue = DispatchQueue(label: "org.burstframe.deduplicator.decisions", qos: .userInitiated)
    private var decisionGenerations: [String: Int] = [:]
    private var assetIndex: [String: AssetRecord] = [:]
    private var pendingRelocation: DispatchWorkItem?

    init(bridge: RustBridge = RustBridge()) {
        self.bridge = bridge
        resultsRootPath = UserDefaults.standard.string(forKey: "resultsRootPath")
            ?? RunCacheManager.defaultRunsDirectory.path
        appearanceMode = AppearanceMode(
            rawValue: UserDefaults.standard.string(forKey: "appearanceMode") ?? "system"
        ) ?? .system
        defaultMoveDestinationPath = UserDefaults.standard.string(forKey: "defaultMoveDestinationPath")
        if let stored = UserDefaults.standard.data(forKey: "scanOptions"),
           let decoded = try? JSONDecoder().decode(ScanOptions.self, from: stored)
        {
            options = decoded
        } else if let defaults = try? bridge.defaultOptions() {
            options = defaults
        }
    }

    var assetsByID: [String: AssetRecord] {
        assetIndex
    }

    var visibleStacks: [BurstStack] {
        guard let payload else { return [] }
        let query = searchText.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        return payload.manifest.clusters
            .filter { stack in
                let assets = stack.assetIds.compactMap { assetsByID[$0] }
                if filter == .multi && assets.count <= 1 { return false }
                return assets.contains { asset in
                    let action = finalAction(for: asset)
                    let queryMatches = query.isEmpty || asset.representative.relPath.lowercased().contains(query)
                    let filterMatches: Bool
                    switch filter {
                    case .all, .multi: filterMatches = true
                    case .review: filterMatches = action == .review
                    case .keep: filterMatches = action == .keep
                    case .reject: filterMatches = action == .reject
                    case .moved: filterMatches = isMoved(asset)
                    }
                    return queryMatches && filterMatches
                }
            }
            .sorted {
                let leftExpanded = expandedStackIDs.contains($0.id)
                let rightExpanded = expandedStackIDs.contains($1.id)
                if leftExpanded != rightExpanded { return leftExpanded && !rightExpanded }
                return $0.id < $1.id
            }
    }

    var counts: [FrameDecision: Int] {
        var result: [FrameDecision: Int] = [.keep: 0, .reject: 0, .review: 0]
        for asset in payload?.manifest.assets ?? [] {
            result[finalAction(for: asset), default: 0] += 1
        }
        return result
    }

    var movedAssetIDs: Set<String> {
        Set(payload?.moveStatus.activeAssetIds ?? [])
    }

    var activeMovedCount: Int {
        payload?.moveStatus.activeAssetIds.count ?? 0
    }

    var movableRejectCount: Int {
        (payload?.manifest.assets ?? []).filter {
            finalAction(for: $0) == .reject && !isMoved($0)
        }.count
    }

    func isMoved(_ asset: AssetRecord) -> Bool {
        movedAssetIDs.contains(asset.id)
    }

    func startScan() {
        guard let sourceURL else { return }
        let destination = automaticRunDirectory()
        outputURL = destination
        phase = .scanning
        progress = nil
        errorMessage = nil
        let options = options
        DispatchQueue.global(qos: .userInitiated).async { [bridge] in
            do {
                let response = try bridge.scan(
                    root: sourceURL.path,
                    output: destination.path,
                    options: options
                ) { [weak self] update in
                    DispatchQueue.main.async {
                        self?.progress = update
                    }
                }
                let payload = try bridge.loadRun(at: response.runDir)
                DispatchQueue.main.async { [weak self] in
                    self?.install(payload)
                }
            } catch {
                DispatchQueue.main.async { [weak self] in
                    self?.phase = .setup
                    self?.errorMessage = error.localizedDescription
                }
            }
        }
    }

    func startScan(from source: URL) {
        sourceURL = source
        startScan()
    }

    func openRun(at directory: URL) {
        errorMessage = nil
        DispatchQueue.global(qos: .userInitiated).async { [bridge] in
            do {
                let payload = try bridge.loadRun(at: directory.path)
                DispatchQueue.main.async { [weak self] in self?.install(payload) }
            } catch {
                DispatchQueue.main.async { [weak self] in self?.errorMessage = error.localizedDescription }
            }
        }
    }

    func resetForNewScan() {
        pendingRelocation?.cancel()
        phase = .setup
        sourceURL = nil
        payload = nil
        progress = nil
        outputURL = nil
        manualDecisions.removeAll()
        expandedStackIDs.removeAll()
        searchText = ""
        filter = .all
        viewerAssetID = nil
    }

    func finalAction(for asset: AssetRecord) -> FrameDecision {
        if let decision = manualDecisions[asset.id], let action = FrameDecision(rawValue: decision) {
            return action
        }
        return FrameDecision(rawValue: asset.suggestion.action) ?? .review
    }

    func setDecision(_ decision: FrameDecision, for asset: AssetRecord) {
        guard !fileOperationInProgress, !relocationInProgress else { return }
        let assetID = asset.id
        let previous = manualDecisions[asset.id]
        let suggested = FrameDecision(rawValue: asset.suggestion.action) ?? .review
        let override = decision == suggested ? nil : decision.rawValue
        if let override {
            manualDecisions[asset.id] = override
        } else {
            manualDecisions.removeValue(forKey: asset.id)
        }
        autoCollapseStack(asset.clusterId)
        let generation = (decisionGenerations[assetID] ?? 0) + 1
        decisionGenerations[assetID] = generation
        guard let runDirectory = payload?.runDir else { return }
        decisionQueue.async { [bridge, weak self] in
            do {
                let updated = try bridge.setDecision(
                    runDirectory: runDirectory,
                    assetID: assetID,
                    decision: override
                )
                DispatchQueue.main.async {
                    guard self?.decisionGenerations[assetID] == generation else { return }
                    self?.payload = updated
                }
            } catch {
                DispatchQueue.main.async {
                    guard self?.decisionGenerations[assetID] == generation else { return }
                    if let previous {
                        self?.manualDecisions[assetID] = previous
                    } else {
                        self?.manualDecisions.removeValue(forKey: assetID)
                    }
                    self?.errorMessage = error.localizedDescription
                }
            }
        }
    }

    func resetDecision(for asset: AssetRecord) {
        setDecision(FrameDecision(rawValue: asset.suggestion.action) ?? .review, for: asset)
    }

    func toggleStack(_ stackID: Int) {
        withAnimation(.snappy(duration: 0.24)) {
            if expandedStackIDs.contains(stackID) {
                expandedStackIDs.remove(stackID)
            } else {
                expandedStackIDs.insert(stackID)
            }
        }
    }

    func stackAssets(_ stack: BurstStack) -> [AssetRecord] {
        stack.assetIds.compactMap { assetsByID[$0] }.filter { asset in
            let query = searchText.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
            if !query.isEmpty && !asset.representative.relPath.lowercased().contains(query) { return false }
            switch filter {
            case .all, .multi: return true
            case .review: return finalAction(for: asset) == .review
            case .keep: return finalAction(for: asset) == .keep
            case .reject: return finalAction(for: asset) == .reject
            case .moved: return isMoved(asset)
            }
        }
    }

    func showRunFolder() {
        guard let runDirectory = payload?.runDir else { return }
        NSWorkspace.shared.activateFileViewerSelecting([URL(fileURLWithPath: runDirectory)])
    }

    func changeResultsRoot(to directory: URL) {
        guard !relocationInProgress else { return }
        let normalized = directory.standardizedFileURL.path
        guard normalized != resultsRootPath else { return }
        resultsRootPath = normalized
        UserDefaults.standard.set(normalized, forKey: "resultsRootPath")
        pendingRelocation?.cancel()
        guard phase == .review, payload != nil else { return }
        let work = DispatchWorkItem { [weak self] in
            self?.relocateCurrentRun(to: normalized)
        }
        pendingRelocation = work
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.75, execute: work)
    }

    func resetResultsRoot() {
        changeResultsRoot(to: RunCacheManager.defaultRunsDirectory)
    }

    func moveRejects(destination: URL?) {
        guard let runDirectory = payload?.runDir else { return }
        let selectedDestination = destination?.path ?? defaultMoveDestinationPath
        fileOperationInProgress = true
        DispatchQueue.global(qos: .userInitiated).async { [bridge] in
            do {
                let result = try bridge.moveRejects(
                    runDirectory: runDirectory,
                    destination: selectedDestination,
                    confirmed: true
                )
                let updated = try bridge.loadRun(at: runDirectory)
                DispatchQueue.main.async { [weak self] in
                    self?.refresh(updated)
                    self?.fileOperationInProgress = false
                    if !result.sourceAvailable {
                        self?.notice = .sourceUnavailable(result.message ?? "Source folder unavailable")
                    } else {
                        self?.notice = .moved(
                            files: result.movedFiles,
                            assets: result.movedAssets,
                            destination: result.destination,
                            failures: result.failedFiles.count
                        )
                    }
                }
            } catch {
                DispatchQueue.main.async { [weak self] in
                    self?.fileOperationInProgress = false
                    self?.errorMessage = error.localizedDescription
                }
            }
        }
    }

    func restoreMoved() {
        guard let runDirectory = payload?.runDir else { return }
        fileOperationInProgress = true
        DispatchQueue.global(qos: .userInitiated).async { [bridge] in
            do {
                let result = try bridge.restoreRejects(runDirectory: runDirectory, confirmed: true)
                let updated = try bridge.loadRun(at: runDirectory)
                DispatchQueue.main.async { [weak self] in
                    self?.refresh(updated)
                    self?.fileOperationInProgress = false
                    if !result.sourceAvailable {
                        self?.notice = .sourceUnavailable(result.message ?? "Original source folder unavailable")
                    } else {
                        self?.notice = .restored(
                            files: result.restoredFiles,
                            assets: result.restoredAssets,
                            failures: result.failedFiles.count
                        )
                    }
                }
            } catch {
                DispatchQueue.main.async { [weak self] in
                    self?.fileOperationInProgress = false
                    self?.errorMessage = error.localizedDescription
                }
            }
        }
    }

    func adjacentAsset(from assetID: String, delta: Int) -> String? {
        guard let payload,
              let asset = assetsByID[assetID],
              let stack = payload.manifest.clusters.first(where: { $0.id == asset.clusterId }),
              let index = stack.assetIds.firstIndex(of: assetID)
        else { return nil }
        let next = max(0, min(stack.assetIds.count - 1, index + delta))
        return next == index ? nil : stack.assetIds[next]
    }

    private func install(_ payload: ReviewPayload) {
        self.payload = payload
        sourceURL = URL(fileURLWithPath: payload.manifest.root)
        outputURL = URL(fileURLWithPath: payload.runDir)
        manualDecisions = Dictionary(uniqueKeysWithValues: payload.review.decisions.compactMap {
            guard let decision = $0.decision else { return nil }
            return ($0.assetId, decision)
        })
        expandedStackIDs = Set(payload.manifest.clusters.filter { $0.assetIds.count > 1 }.map(\.id))
        for stack in payload.manifest.clusters {
            autoCollapseStack(stack.id)
        }
        phase = .review
        RunCacheManager.registerRun(payload.runDir)
    }

    private func refresh(_ payload: ReviewPayload) {
        self.payload = payload
        manualDecisions = Dictionary(uniqueKeysWithValues: payload.review.decisions.compactMap {
            guard let decision = $0.decision else { return nil }
            return ($0.assetId, decision)
        })
    }

    private func autoCollapseStack(_ stackID: Int) {
        guard let stack = payload?.manifest.clusters.first(where: { $0.id == stackID }) else { return }
        let allKept = stack.assetIds
            .compactMap { assetsByID[$0] }
            .allSatisfy { finalAction(for: $0) == .keep }
        if allKept {
            expandedStackIDs.remove(stackID)
        }
    }

    private func automaticRunDirectory() -> URL {
        let formatter = DateFormatter()
        formatter.dateFormat = "yyyyMMdd_HHmmss_SSS"
        let suffix = UUID().uuidString.prefix(6).lowercased()
        return URL(fileURLWithPath: resultsRootPath, isDirectory: true)
            .appendingPathComponent("run_\(formatter.string(from: Date()))_\(suffix)", isDirectory: true)
    }

    private func relocateCurrentRun(to destinationRoot: String) {
        guard let currentRun = payload?.runDir, !relocationInProgress else { return }
        let currentParent = URL(fileURLWithPath: currentRun).deletingLastPathComponent().standardizedFileURL.path
        if currentParent == destinationRoot { return }

        relocationInProgress = true
        fileOperationInProgress = true
        relocationProgress = nil
        errorMessage = nil
        DispatchQueue.global(qos: .userInitiated).async { [bridge] in
            do {
                let result = try bridge.relocateRun(
                    runDirectory: currentRun,
                    destinationRoot: destinationRoot
                ) { [weak self] update in
                    DispatchQueue.main.async { self?.relocationProgress = update }
                }
                let updated = try bridge.loadRun(at: result.runDir)
                DispatchQueue.main.async { [weak self] in
                    guard let self else { return }
                    RunCacheManager.replaceRegisteredRun(
                        previous: result.previousRunDir,
                        relocated: result.runDir
                    )
                    payload = updated
                    outputURL = URL(fileURLWithPath: result.runDir)
                    relocationInProgress = false
                    fileOperationInProgress = false
                    relocationProgress = nil
                    if !result.warnings.isEmpty {
                        notice = .message(result.warnings.joined(separator: "\n"))
                    }
                }
            } catch {
                DispatchQueue.main.async { [weak self] in
                    guard let self else { return }
                    resultsRootPath = currentParent
                    UserDefaults.standard.set(currentParent, forKey: "resultsRootPath")
                    relocationInProgress = false
                    fileOperationInProgress = false
                    relocationProgress = nil
                    errorMessage = error.localizedDescription
                }
            }
        }
    }

    private func persistOptions() {
        guard let data = try? JSONEncoder().encode(options) else { return }
        UserDefaults.standard.set(data, forKey: "scanOptions")
    }
}
