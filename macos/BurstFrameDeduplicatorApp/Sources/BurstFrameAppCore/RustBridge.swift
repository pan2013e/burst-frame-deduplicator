import CBurstFrameDeduplicator
import Foundation

public enum RustBridgeError: LocalizedError {
    case encoding(String)
    case backend(String)
    case invalidResponse(String)

    public var errorDescription: String? {
        switch self {
        case .encoding(let message), .backend(let message), .invalidResponse(let message):
            return message
        }
    }
}

private final class ProgressContext {
    let handler: (ProgressUpdate) -> Void

    init(handler: @escaping (ProgressUpdate) -> Void) {
        self.handler = handler
    }
}

private let bridgeProgressCallback: @convention(c) (UnsafePointer<CChar>?, UnsafeMutableRawPointer?) -> Void = {
    jsonPointer, contextPointer in
    guard let jsonPointer, let contextPointer else { return }
    let data = Data(String(cString: jsonPointer).utf8)
    let decoder = RustBridge.makeDecoder()
    guard let progress = try? decoder.decode(ProgressUpdate.self, from: data) else { return }
    Unmanaged<ProgressContext>.fromOpaque(contextPointer).takeUnretainedValue().handler(progress)
}

public final class ScanCancellation: @unchecked Sendable {
    fileprivate let id: UInt64

    public init() {
        id = bfd_scan_control_create()
    }

    public func cancel() {
        _ = bfd_scan_control_cancel(id)
    }

    deinit {
        bfd_scan_control_release(id)
    }
}

public final class RustBridge: @unchecked Sendable {
    public init() {}

    public var apiVersion: UInt32 { bfd_api_version() }

    public func defaultOptions() throws -> ScanOptions {
        try decodeResponse(bfd_default_options())
    }

    public func scan(
        root: String,
        output: String,
        options: ScanOptions,
        cancellation: ScanCancellation? = nil,
        progress: @escaping (ProgressUpdate) -> Void
    ) throws -> ScanResponse {
        let request = ScanRequest(root: root, out: output, options: options)
        let encoded = try encode(request)
        let context = Unmanaged.passRetained(ProgressContext(handler: progress))
        defer { context.release() }
        let response = encoded.withCString { pointer in
            if let cancellation {
                bfd_scan_controlled(
                    pointer,
                    bridgeProgressCallback,
                    context.toOpaque(),
                    cancellation.id
                )
            } else {
                bfd_scan(pointer, bridgeProgressCallback, context.toOpaque())
            }
        }
        return try decodeResponse(response)
    }

    public func loadRun(
        at runDirectory: String,
        progress: ((ProgressUpdate) -> Void)? = nil
    ) throws -> ReviewPayload {
        guard let progress else {
            return try invoke(RunRequest(runDir: runDirectory), function: bfd_load_run)
        }
        let encoded = try encode(RunRequest(runDir: runDirectory))
        let context = Unmanaged.passRetained(ProgressContext(handler: progress))
        defer { context.release() }
        let response = encoded.withCString { pointer in
            bfd_load_run_with_progress(pointer, bridgeProgressCallback, context.toOpaque())
        }
        return try decodeResponse(response)
    }

    public func setDecision(
        runDirectory: String,
        assetID: String,
        decision: String?
    ) throws -> ReviewPayload {
        try invoke(
            DecisionRequest(runDir: runDirectory, assetId: assetID, decision: decision),
            function: bfd_set_decision
        )
    }

    public func preparePreview(
        runDirectory: String,
        assetID: String,
        maxLongEdge: UInt32 = 4096,
        generateIfMissing: Bool = true
    ) throws -> PreviewResponse {
        try invoke(
            PreviewRequest(
                runDir: runDirectory,
                assetId: assetID,
                maxLongEdge: maxLongEdge,
                generateIfMissing: generateIfMissing
            ),
            function: bfd_prepare_preview
        )
    }

    public func exportRun(at runDirectory: String) throws -> ReviewPayload {
        try invoke(RunRequest(runDir: runDirectory), function: bfd_export_run)
    }

    public func moveRejects(
        runDirectory: String,
        destination: String?,
        confirmed: Bool
    ) throws -> MoveResponse {
        try invoke(
            MoveRequest(runDir: runDirectory, destination: destination, confirmed: confirmed),
            function: bfd_move_rejects
        )
    }

    public func restoreRejects(
        runDirectory: String,
        assetIDs: [String]? = nil,
        confirmed: Bool
    ) throws -> RestoreResponse {
        try invoke(
            RestoreRequest(runDir: runDirectory, assetIds: assetIDs, confirmed: confirmed),
            function: bfd_restore_rejects
        )
    }

    public func planCounterparts(
        runDirectory: String,
        cardRoot: String
    ) throws -> CounterpartPlan {
        try invoke(
            CounterpartPlanRequest(runDir: runDirectory, cardRoot: cardRoot),
            function: bfd_plan_counterparts
        )
    }

    public func applyCounterparts(
        runDirectory: String,
        cardRoot: String,
        destination: String?,
        confirmed: Bool
    ) throws -> CounterpartMoveResponse {
        try invoke(
            CounterpartMoveRequest(
                runDir: runDirectory,
                cardRoot: cardRoot,
                destination: destination,
                confirmed: confirmed
            ),
            function: bfd_apply_counterparts
        )
    }

    public func restoreCounterparts(
        runDirectory: String,
        cardRoot: String,
        confirmed: Bool
    ) throws -> RestoreResponse {
        try invoke(
            CounterpartRestoreRequest(
                runDir: runDirectory,
                cardRoot: cardRoot,
                confirmed: confirmed
            ),
            function: bfd_restore_counterparts
        )
    }

    public func relocateRun(
        runDirectory: String,
        destinationRoot: String,
        progress: @escaping (ProgressUpdate) -> Void
    ) throws -> RelocationResponse {
        let request = RelocateRequest(runDir: runDirectory, destinationRoot: destinationRoot)
        let encoded = try encode(request)
        let context = Unmanaged.passRetained(ProgressContext(handler: progress))
        defer { context.release() }
        let response = encoded.withCString { pointer in
            bfd_relocate_run(pointer, bridgeProgressCallback, context.toOpaque())
        }
        return try decodeResponse(response)
    }

    private func invoke<Request: Encodable, Response: Decodable>(
        _ request: Request,
        function: (UnsafePointer<CChar>?) -> UnsafeMutablePointer<CChar>?
    ) throws -> Response {
        let encoded = try encode(request)
        let response = encoded.withCString { function($0) }
        return try decodeResponse(response)
    }

    private func encode<Value: Encodable>(_ value: Value) throws -> String {
        do {
            let encoder = JSONEncoder()
            encoder.keyEncodingStrategy = .convertToSnakeCase
            let data = try encoder.encode(value)
            guard let json = String(data: data, encoding: .utf8) else {
                throw RustBridgeError.encoding("Request JSON is not UTF-8")
            }
            return json
        } catch let error as RustBridgeError {
            throw error
        } catch {
            throw RustBridgeError.encoding(error.localizedDescription)
        }
    }

    private func decodeResponse<Value: Decodable>(
        _ pointer: UnsafeMutablePointer<CChar>?
    ) throws -> Value {
        guard let pointer else {
            throw RustBridgeError.invalidResponse("Rust backend returned a null response")
        }
        defer { bfd_free_string(pointer) }
        let data = Data(String(cString: pointer).utf8)
        let envelope: BridgeEnvelope<Value>
        do {
            envelope = try Self.makeDecoder().decode(BridgeEnvelope<Value>.self, from: data)
        } catch {
            throw RustBridgeError.invalidResponse(error.localizedDescription)
        }
        guard envelope.ok, let value = envelope.value else {
            throw RustBridgeError.backend(envelope.error ?? "Rust backend returned an error")
        }
        return value
    }

    fileprivate static func makeDecoder() -> JSONDecoder {
        let decoder = JSONDecoder()
        decoder.keyDecodingStrategy = .convertFromSnakeCase
        return decoder
    }
}

private struct ScanRequest: Encodable {
    let root: String
    let out: String
    let options: ScanOptions
}

private struct RunRequest: Encodable {
    let runDir: String
}

private struct DecisionRequest: Encodable {
    let runDir: String
    let assetId: String
    let decision: String?
}

private struct PreviewRequest: Encodable {
    let runDir: String
    let assetId: String
    let maxLongEdge: UInt32
    let generateIfMissing: Bool
}

private struct MoveRequest: Encodable {
    let runDir: String
    let destination: String?
    let confirmed: Bool
}

private struct RestoreRequest: Encodable {
    let runDir: String
    let assetIds: [String]?
    let confirmed: Bool
}

private struct RelocateRequest: Encodable {
    let runDir: String
    let destinationRoot: String
}

private struct CounterpartPlanRequest: Encodable {
    let runDir: String
    let cardRoot: String
}

private struct CounterpartMoveRequest: Encodable {
    let runDir: String
    let cardRoot: String
    let destination: String?
    let confirmed: Bool
}

private struct CounterpartRestoreRequest: Encodable {
    let runDir: String
    let cardRoot: String
    let confirmed: Bool
}
