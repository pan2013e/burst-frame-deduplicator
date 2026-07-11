import BurstFrameAppCore
import Foundation

private struct Arguments {
    var source: String
    var output: String
    var acceleration = "auto"
    var detector = "heuristic"
    var maxTimeGapMs: Int64 = 1_250
    var maxClusterSpanMs: Int64 = 1_800
    var workers: Int?

    init(_ values: [String]) throws {
        var parsed: [String: String] = [:]
        var index = 0
        while index < values.count {
            let key = values[index]
            guard key.hasPrefix("--"), index + 1 < values.count else {
                throw BenchmarkError.usage
            }
            parsed[key] = values[index + 1]
            index += 2
        }
        guard let source = parsed["--source"], let output = parsed["--out"] else {
            throw BenchmarkError.usage
        }
        self.source = source
        self.output = output
        acceleration = parsed["--acceleration"] ?? acceleration
        detector = parsed["--detector"] ?? detector
        if let value = parsed["--max-time-gap-ms"] { maxTimeGapMs = Int64(value) ?? maxTimeGapMs }
        if let value = parsed["--max-cluster-span-ms"] { maxClusterSpanMs = Int64(value) ?? maxClusterSpanMs }
        if let value = parsed["--workers"] { workers = Int(value) }
    }
}

private enum BenchmarkError: LocalizedError {
    case usage

    var errorDescription: String? {
        "Usage: BurstFrameFFIBenchmark --source <folder> --out <run> [--acceleration auto|cpu|metal] [--detector heuristic|vision|off] [--max-time-gap-ms N] [--max-cluster-span-ms N] [--workers N]"
    }
}

private struct BenchmarkOutput: Encodable {
    let path: String
    let elapsedMs: Double
    let assets: Int
    let assetsPerSecond: Double
    let acceleration: String
    let detector: String
    let stages: [StageOutput]
}

private struct StageOutput: Encodable {
    let stage: String
    let elapsedMs: Double
    let itemsPerSecond: Double?
}

do {
    let arguments = try Arguments(Array(CommandLine.arguments.dropFirst()))
    let bridge = RustBridge()
    var options = try bridge.defaultOptions()
    options.acceleration = arguments.acceleration
    options.detector = arguments.detector
    options.maxTimeGapMs = arguments.maxTimeGapMs
    options.maxClusterSpanMs = arguments.maxClusterSpanMs
    options.workers = arguments.workers
    let started = ContinuousClock.now
    let result = try bridge.scan(
        root: arguments.source,
        output: arguments.output,
        options: options
    ) { progress in
        FileHandle.standardError.write(
            Data("\r[\(progress.stage)] \(Int(progress.overallFraction * 100))%".utf8)
        )
    }
    FileHandle.standardError.write(Data("\n".utf8))
    let elapsed = started.duration(to: .now)
    let elapsedMs = Double(elapsed.components.seconds) * 1_000
        + Double(elapsed.components.attoseconds) / 1_000_000_000_000_000
    let payload = try bridge.loadRun(at: result.runDir)
    let assets = payload.manifest.summary.discoveredAssets
    let output = BenchmarkOutput(
        path: result.runDir,
        elapsedMs: elapsedMs,
        assets: assets,
        assetsPerSecond: elapsedMs > 0 ? Double(assets) * 1_000 / elapsedMs : 0,
        acceleration: payload.manifest.acceleration.selected,
        detector: payload.manifest.detector.selected,
        stages: payload.manifest.benchmarks.map {
            StageOutput(stage: $0.stage, elapsedMs: $0.elapsedMs, itemsPerSecond: $0.itemsPerSec)
        }
    )
    let encoder = JSONEncoder()
    encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
    FileHandle.standardOutput.write(try encoder.encode(output))
    FileHandle.standardOutput.write(Data("\n".utf8))
} catch {
    FileHandle.standardError.write(Data("\(error.localizedDescription)\n".utf8))
    exit(2)
}
