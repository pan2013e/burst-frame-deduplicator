import Foundation

public struct BridgeEnvelope<Value: Decodable>: Decodable {
    public let ok: Bool
    public let value: Value?
    public let error: String?
}

public struct EmptyValue: Decodable {}

public struct ScanOptions: Codable, Equatable, Sendable {
    public var previewSize: UInt32 = 1280
    public var refineSize: UInt32 = 2048
    public var refineCandidatesPerCluster: Int = 2
    public var disableRefinement = false
    public var thumbSize: UInt32 = 320
    public var maxSeqGap: Int64 = 12
    public var maxTimeGapMs: Int64 = 1250
    public var maxClusterSpanMs: Int64 = 1800
    public var maxHashGap: UInt32 = 30
    public var maxDuplicateDistance = 0.20
    public var minDuplicateConfidence = 0.52
    public var keepersPerCluster: Int?
    public var cullSingletons = false
    public var workers: Int?
    public var acceleration = "auto"
    public var detector = "auto"
    public var generateThumbnails = true

    public init() {}
}

public struct ProgressUpdate: Decodable, Sendable {
    public let stage: String
    public let current: Int
    public let total: Int?
    public let stageFraction: Double?
    public let overallFraction: Double
    public let detail: String?
}

public struct ScanResponse: Decodable {
    public let runDir: String
}

public struct ReviewPayload: Decodable {
    public let runDir: String
    public let manifest: RunManifest
    public let review: ReviewState
    public let moveStatus: MoveStatus
}

public struct PreviewResponse: Decodable {
    public let path: String
    public let generated: Bool
}

public struct MoveResponse: Decodable {
    public let destination: String
    public let movedFiles: Int
    public let movedAssets: Int
    public let alreadyMovedAssets: Int
    public let movedAssetIds: [String]
    public let sourceAvailable: Bool
    public let missingFiles: [String]
    public let failedFiles: [MoveFailure]
    public let message: String?
    public let status: MoveStatus
}

public struct RestoreResponse: Decodable {
    public let restoredFiles: Int
    public let restoredAssets: Int
    public let restoredAssetIds: [String]
    public let sourceAvailable: Bool
    public let missingFiles: [String]
    public let failedFiles: [MoveFailure]
    public let message: String?
    public let status: MoveStatus
}

public struct MoveStatus: Decodable, Equatable, Sendable {
    public let activeAssetIds: [String]
    public let activeFiles: Int
    public let activeBytes: UInt64
    public let destinations: [String]
}

public struct MoveFailure: Decodable {
    public let source: String
    public let error: String
}

public struct RunManifest: Decodable {
    public let root: String
    public let createdAt: String
    public let acceleration: BackendReport
    public let detector: BackendReport
    public let benchmarks: [BenchmarkRecord]
    public let summary: RunSummary
    public let bursts: [BurstSequence]
    public let clusters: [BurstStack]
    public let assets: [AssetRecord]
}

public struct BackendReport: Decodable {
    public let requested: String
    public let selected: String
    public let capabilities: [String]
    public let notes: [String]
}

public struct BenchmarkRecord: Decodable {
    public let stage: String
    public let elapsedMs: Double
    public let items: Int?
    public let itemsPerSec: Double?
}

public struct RunSummary: Decodable {
    public let discoveredAssets: Int
    public let clusters: Int
    public let bursts: Int
    public let suggestedKeep: Int
    public let suggestedReject: Int
    public let suggestedReview: Int
    public let errors: Int
}

public struct BurstSequence: Decodable, Identifiable {
    public let id: Int
    public let assetIds: [String]
    public let clusterIds: [Int]
}

public struct BurstStack: Decodable, Identifiable {
    public let id: Int
    public let burstId: Int
    public let assetIds: [String]
    public let keepCount: Int
    public let bestAssetId: String?
    public let similarityConfidence: Double
    public let maxDistance: Double
}

public struct AssetRecord: Decodable, Identifiable {
    public let id: String
    public let representative: FileEntry
    public let files: [FileEntry]
    public let sidecars: [FileEntry]
    public let width: UInt32
    public let height: UInt32
    public let decoder: String
    public let featureBackend: String
    public let metadata: PhotoMetadata
    public let metrics: QualityMetrics
    public let detector: DetectorOutput?
    public let burstId: Int
    public let clusterId: Int
    public let similarity: SimilarityMetrics
    public let suggestion: Suggestion
    public let thumb: String?
    public let error: String?
}

public struct FileEntry: Decodable {
    public let path: String
    public let relPath: String
    public let kind: String
    public let `extension`: String
}

public struct PhotoMetadata: Decodable {
    public let iso: UInt32?
    public let aperture: Double?
    public let shutter: String?
    public let focalLengthMm: Double?
    public let focalLength35mm: UInt32?
}

public struct QualityMetrics: Decodable {
    public let sharpness: Double
    public let subjectSharpness: Double
    public let contrast: Double
    public let exposureScore: Double
    public let clippedFraction: Double
    public let completeness: Double
    public let objectConfidence: Double
    public let borderEnergyFraction: Double
}

public struct SimilarityMetrics: Decodable {
    public let nearestDistance: Double
    public let nearestSubjectDistance: Double
    public let nearestGlobalDistance: Double
    public let duplicateConfidence: Double
    public let poseNovelty: Double
}

public struct DetectorOutput: Decodable {
    public let backend: String
    public let confidence: Double
    public let truncationRisk: Double
}

public struct Suggestion: Decodable {
    public let action: String
    public let rank: Int
    public let score: Double
    public let reason: String
    public let explanations: [String]
}

public struct ReviewState: Decodable {
    public let runCreatedAt: String
    public let updatedAt: String
    public let decisions: [ReviewDecision]
}

public struct ReviewDecision: Decodable {
    public let assetId: String
    public let decision: String?
    public let updatedAt: String
}
