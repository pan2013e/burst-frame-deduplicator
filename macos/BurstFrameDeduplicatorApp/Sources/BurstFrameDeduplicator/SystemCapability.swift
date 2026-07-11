import BurstFrameAppCore
import Darwin
import Foundation
import Metal
import SwiftUI

struct SystemCapability: Sendable {
    static let current = SystemCapability()

    let modelName: String
    let cpuName: String
    let logicalCPUCount: Int
    let memoryBytes: UInt64
    let gpuName: String
    let metalFamily: String
    let osVersion: String

    init() {
        let process = ProcessInfo.processInfo
        modelName = Self.sysctlString("hw.model") ?? "Mac"
        cpuName = Self.sysctlString("machdep.cpu.brand_string") ?? "Apple Silicon"
        logicalCPUCount = process.activeProcessorCount
        memoryBytes = process.physicalMemory
        osVersion = process.operatingSystemVersionString
        if let device = MTLCreateSystemDefaultDevice() {
            gpuName = device.name
            if #available(macOS 26.0, *), device.supportsFamily(.metal4) {
                metalFamily = "Metal 4"
            } else if device.supportsFamily(.metal3) {
                metalFamily = "Metal 3"
            } else {
                metalFamily = "Metal (legacy family)"
            }
        } else {
            gpuName = "Unavailable"
            metalFamily = "Unavailable"
        }
    }

    var capabilityScore: Double {
        let cpu = min(1.0, Double(logicalCPUCount) / 12.0)
        let memoryGB = Double(memoryBytes) / 1_073_741_824.0
        let memory = min(1.0, memoryGB / 24.0)
        let gpu = gpuName == "Unavailable" ? 0.0 : 0.9
        return (cpu * 0.45 + memory * 0.30 + gpu * 0.25).clamped(to: 0...1)
    }

    func estimatedLoad(for options: ScanOptions) -> Double {
        let previewPixels = pow(Double(options.previewSize) / 1280.0, 2)
        let refinementPixels = options.disableRefinement
            ? 0
            : pow(Double(options.refineSize) / 2048.0, 2)
                * Double(options.refineCandidatesPerCluster) / 2.0
        let detectorCost: Double = switch options.detector {
        case "vision": 0.55
        case "auto": 0.42
        case "heuristic": 0.14
        default: 0.0
        }
        let accelerationBenefit = options.acceleration == "cpu" ? 1.0 : 0.82
        let workload = (previewPixels * 0.48 + refinementPixels * 0.38 + detectorCost) * accelerationBenefit
        let capacity = 0.55 + capabilityScore * 0.95
        return (workload / capacity / 1.6).clamped(to: 0...1)
    }

    private static func sysctlString(_ name: String) -> String? {
        var size = 0
        guard sysctlbyname(name, nil, &size, nil, 0) == 0, size > 0 else { return nil }
        var value = [CChar](repeating: 0, count: size)
        guard sysctlbyname(name, &value, &size, nil, 0) == 0 else { return nil }
        return String(cString: value)
    }
}

struct BuildDiagnostics {
    let commit: String
    let rustVersion: String
    let swiftVersion: String
    let commandLineToolsVersion: String
    let appVersion: String

    static var current: BuildDiagnostics {
        let info = Bundle.main.infoDictionary ?? [:]
        return BuildDiagnostics(
            commit: info["BFDGitCommit"] as? String ?? "development",
            rustVersion: info["BFDRustVersion"] as? String ?? "development",
            swiftVersion: info["BFDSwiftVersion"] as? String ?? "development",
            commandLineToolsVersion: info["BFDCLTVersion"] as? String ?? "development",
            appVersion: info["CFBundleShortVersionString"] as? String ?? "0.1.1"
        )
    }
}

struct ContinuousLevelBar: View {
    let value: Double
    var colors: [Color] = [.red, .orange, .yellow, .green]

    var body: some View {
        GeometryReader { proxy in
            let position = max(3, min(proxy.size.width - 3, proxy.size.width * value.clamped(to: 0...1)))
            ZStack(alignment: .leading) {
                Capsule()
                    .fill(.quaternary.opacity(0.62))
                Capsule()
                    .fill(LinearGradient(colors: colors, startPoint: .leading, endPoint: .trailing))
                    .opacity(0.34)
                Capsule()
                    .strokeBorder(.primary.opacity(0.06), lineWidth: 0.5)
                Circle()
                    .fill(.background)
                    .overlay(Circle().stroke(.primary.opacity(0.42), lineWidth: 0.75))
                    .frame(width: 6, height: 6)
                    .offset(x: position - 3)
            }
        }
        .frame(height: 6)
        .accessibilityValue(Text(value, format: .percent.precision(.fractionLength(0))))
    }
}

private extension Double {
    func clamped(to range: ClosedRange<Double>) -> Double {
        max(range.lowerBound, min(range.upperBound, self))
    }
}
