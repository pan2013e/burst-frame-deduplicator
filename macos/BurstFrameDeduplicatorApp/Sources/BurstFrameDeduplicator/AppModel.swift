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
    case multi

    var id: String { rawValue }
}

@MainActor
final class AppModel: ObservableObject {
    @Published var phase: AppPhase = .setup
    @Published var sourceURL: URL?
    @Published var outputURL: URL?
    @Published var options = ScanOptions()
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
    @Published var noticeMessage: String?

    private let bridge: RustBridge
    private let decisionQueue = DispatchQueue(label: "org.burstframe.deduplicator.decisions", qos: .userInitiated)
    private var decisionGenerations: [String: Int] = [:]
    private var assetIndex: [String: AssetRecord] = [:]

    init(bridge: RustBridge = RustBridge()) {
        self.bridge = bridge
        if let defaults = try? bridge.defaultOptions() {
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

    func startScan() {
        guard let sourceURL else { return }
        let destination = outputURL ?? automaticRunDirectory()
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
        phase = .setup
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
            }
        }
    }

    func showRunFolder() {
        guard let runDirectory = payload?.runDir else { return }
        NSWorkspace.shared.activateFileViewerSelecting([URL(fileURLWithPath: runDirectory)])
    }

    func moveRejects() {
        guard let runDirectory = payload?.runDir else { return }
        DispatchQueue.global(qos: .userInitiated).async { [bridge] in
            do {
                let result = try bridge.moveRejects(runDirectory: runDirectory, confirmed: true)
                DispatchQueue.main.async { [weak self] in
                    self?.noticeMessage = "\(result.movedFiles)|\(result.movedAssets)"
                }
            } catch {
                DispatchQueue.main.async { [weak self] in self?.errorMessage = error.localizedDescription }
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
        formatter.dateFormat = "yyyyMMdd_HHmmss"
        let pictures = FileManager.default.urls(for: .picturesDirectory, in: .userDomainMask).first
            ?? FileManager.default.homeDirectoryForCurrentUser
        return pictures
            .appendingPathComponent("Burst Frame Deduplicator Runs", isDirectory: true)
            .appendingPathComponent("run_\(formatter.string(from: Date()))", isDirectory: true)
    }
}
