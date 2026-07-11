import AppKit
import BurstFrameAppCore
import ImageIO
import SwiftUI

private let previewImageCache: NSCache<NSString, NSImage> = {
    let cache = NSCache<NSString, NSImage>()
    cache.countLimit = 12
    cache.totalCostLimit = 640 * 1024 * 1024
    return cache
}()

private struct CanvasCommand: Equatable {
    enum Action {
        case fit
        case zoomIn
        case zoomOut
    }

    let id = UUID()
    let action: Action
}

struct NativeImageViewer: View {
    @EnvironmentObject private var locale: LocaleCatalog
    @Environment(\.dismiss) private var dismiss
    @ObservedObject var model: AppModel
    @State private var image: NSImage?
    @State private var loading = true
    @State private var loadError: String?
    @State private var canvasCommand = CanvasCommand(action: .fit)
    @FocusState private var focused: Bool
    private let bridge = RustBridge()

    var body: some View {
        VStack(spacing: 0) {
            viewerToolbar
            Divider()
            ZStack {
                Color(nsColor: .underPageBackgroundColor)
                if loading {
                    VStack(spacing: 12) {
                        ProgressView()
                            .controlSize(.large)
                        Text(locale.text("loadingPreview"))
                            .foregroundStyle(.secondary)
                    }
                } else if let loadError {
                    ContentUnavailableView {
                        Label(locale.text("previewUnavailable"), systemImage: "externaldrive.badge.exclamationmark")
                    } description: {
                        Text(loadError)
                    }
                } else if let image {
                    ZoomableImageCanvas(image: image, command: canvasCommand)
                }
            }
        }
        .frame(minWidth: 820, minHeight: 620)
        .focusable()
        .focused($focused)
        .onAppear { focused = true }
        .onKeyPress(.leftArrow) {
            navigate(-1)
            return .handled
        }
        .onKeyPress(.rightArrow) {
            navigate(1)
            return .handled
        }
        .onKeyPress(.escape) {
            close()
            return .handled
        }
        .task(id: model.viewerAssetID) {
            await loadCurrentImage()
        }
    }

    private var viewerToolbar: some View {
        HStack(spacing: 10) {
            Text(currentAsset?.representative.relPath ?? "")
                .font(.headline)
                .lineLimit(1)
                .truncationMode(.middle)
            Spacer()
            if let asset = currentAsset {
                HStack(spacing: 5) {
                    TriStateCheckbox(
                        state: model.finalAction(for: asset),
                        accessibilityLabel: locale.text("keep")
                    ) { model.setDecision($0, for: asset) }
                    .fixedSize()
                    Text(locale.text("keep"))
                }
                if model.isMoved(asset) {
                    Label(locale.text("moved"), systemImage: "tray.full")
                        .font(.callout)
                        .foregroundStyle(.blue)
                }
            }
            Divider().frame(height: 22)
            iconButton("chevron.left", help: locale.text("previousFrame"), disabled: adjacent(-1) == nil) {
                navigate(-1)
            }
            iconButton("chevron.right", help: locale.text("nextFrame"), disabled: adjacent(1) == nil) {
                navigate(1)
            }
            iconButton("minus.magnifyingglass", help: locale.text("zoomOut")) {
                canvasCommand = CanvasCommand(action: .zoomOut)
            }
            iconButton("plus.magnifyingglass", help: locale.text("zoomIn")) {
                canvasCommand = CanvasCommand(action: .zoomIn)
            }
            iconButton("arrow.down.right.and.arrow.up.left", help: locale.text("fit")) {
                canvasCommand = CanvasCommand(action: .fit)
            }
            iconButton("xmark", help: locale.text("close"), action: close)
        }
        .padding(.horizontal, 12)
        .frame(height: 48)
        .background(.bar)
    }

    private var currentAsset: AssetRecord? {
        guard let id = model.viewerAssetID else { return nil }
        return model.assetsByID[id]
    }

    private func iconButton(
        _ symbol: String,
        help: String,
        disabled: Bool = false,
        action: @escaping () -> Void
    ) -> some View {
        Button(action: action) {
            Image(systemName: symbol)
                .frame(width: 18, height: 18)
        }
        .buttonStyle(.borderless)
        .disabled(disabled)
        .accessibilityLabel(Text(help))
        .help(help)
    }

    private func adjacent(_ delta: Int) -> String? {
        guard let id = model.viewerAssetID else { return nil }
        return model.adjacentAsset(from: id, delta: delta)
    }

    private func navigate(_ delta: Int) {
        if let next = adjacent(delta) { model.viewerAssetID = next }
    }

    private func close() {
        model.viewerAssetID = nil
        dismiss()
    }

    private func loadCurrentImage() async {
        guard let asset = currentAsset, let runDirectory = model.payload?.runDir else { return }
        loading = true
        loadError = nil
        image = nil
        let result = await Task.detached(priority: .userInitiated) { () -> Result<NSImage, Error> in
            Result {
                let preview = try bridge.preparePreview(
                    runDirectory: runDirectory,
                    assetID: asset.id,
                    maxLongEdge: 6144
                )
                return try loadDownsampledImage(at: preview.path, maxPixelSize: 6144)
            }
        }.value
        guard model.viewerAssetID == asset.id else { return }
        switch result {
        case .success(let loaded):
            image = loaded
            canvasCommand = CanvasCommand(action: .fit)
        case .failure(let error):
            loadError = error.localizedDescription
        }
        loading = false
    }
}

private func loadDownsampledImage(at path: String, maxPixelSize: Int) throws -> NSImage {
    let cacheKey = "\(path)#\(maxPixelSize)" as NSString
    if let cached = previewImageCache.object(forKey: cacheKey) { return cached }
    let url = URL(fileURLWithPath: path) as CFURL
    guard let source = CGImageSourceCreateWithURL(url, nil) else {
        throw CocoaError(.fileReadCorruptFile, userInfo: [NSFilePathErrorKey: path])
    }
    let options: [CFString: Any] = [
        kCGImageSourceCreateThumbnailFromImageAlways: true,
        kCGImageSourceCreateThumbnailWithTransform: true,
        kCGImageSourceThumbnailMaxPixelSize: maxPixelSize,
        kCGImageSourceShouldCacheImmediately: true,
    ]
    guard let cgImage = CGImageSourceCreateThumbnailAtIndex(source, 0, options as CFDictionary) else {
        throw CocoaError(.fileReadCorruptFile, userInfo: [NSFilePathErrorKey: path])
    }
    let image = NSImage(
        cgImage: cgImage,
        size: NSSize(width: cgImage.width, height: cgImage.height)
    )
    previewImageCache.setObject(
        image,
        forKey: cacheKey,
        cost: cgImage.bytesPerRow * cgImage.height
    )
    return image
}

private final class CenteredClipView: NSClipView {
    override func constrainBoundsRect(_ proposedBounds: NSRect) -> NSRect {
        var bounds = super.constrainBoundsRect(proposedBounds)
        guard let documentView else { return bounds }
        if documentView.frame.width < proposedBounds.width {
            bounds.origin.x = (documentView.frame.width - proposedBounds.width) / 2
        }
        if documentView.frame.height < proposedBounds.height {
            bounds.origin.y = (documentView.frame.height - proposedBounds.height) / 2
        }
        return bounds
    }
}

private final class ViewportTrackingScrollView: NSScrollView {
    var onViewportSizeChange: (() -> Void)?
    private var previousViewportSize = NSSize.zero

    override func layout() {
        super.layout()
        let viewportSize = contentView.frame.size
        guard viewportSize.width > 0,
              viewportSize.height > 0,
              viewportSize != previousViewportSize
        else { return }
        previousViewportSize = viewportSize
        DispatchQueue.main.async { [weak self] in
            self?.onViewportSizeChange?()
        }
    }
}

private struct ZoomableImageCanvas: NSViewRepresentable {
    let image: NSImage
    let command: CanvasCommand

    func makeCoordinator() -> Coordinator {
        Coordinator()
    }

    func makeNSView(context: Context) -> NSScrollView {
        let scrollView = ViewportTrackingScrollView()
        scrollView.contentView = CenteredClipView()
        scrollView.drawsBackground = true
        scrollView.backgroundColor = .underPageBackgroundColor
        scrollView.hasHorizontalScroller = true
        scrollView.hasVerticalScroller = true
        scrollView.autohidesScrollers = true
        scrollView.allowsMagnification = true
        scrollView.automaticallyAdjustsContentInsets = false
        scrollView.scrollerStyle = .overlay
        scrollView.documentView = context.coordinator.imageView
        context.coordinator.connect(to: scrollView)
        return scrollView
    }

    func updateNSView(_ scrollView: NSScrollView, context: Context) {
        let coordinator = context.coordinator
        if coordinator.image !== image {
            coordinator.image = image
            coordinator.imageView.image = image
            let size = imagePixelSize(image)
            coordinator.imageView.frame = NSRect(origin: .zero, size: size)
            coordinator.imageView.needsDisplay = true
            DispatchQueue.main.async { coordinator.fit() }
        }
        if coordinator.lastCommandID != command.id {
            coordinator.lastCommandID = command.id
            DispatchQueue.main.async {
                switch command.action {
                case .fit: coordinator.fit()
                case .zoomIn: coordinator.zoom(by: 1.25)
                case .zoomOut: coordinator.zoom(by: 0.8)
                }
            }
        }
    }

    private func imagePixelSize(_ image: NSImage) -> NSSize {
        let representation = image.representations.max {
            ($0.pixelsWide * $0.pixelsHigh) < ($1.pixelsWide * $1.pixelsHigh)
        }
        guard let representation, representation.pixelsWide > 0, representation.pixelsHigh > 0 else {
            return image.size
        }
        return NSSize(width: representation.pixelsWide, height: representation.pixelsHigh)
    }

    final class Coordinator: NSObject {
        let imageView: NSImageView = {
            let view = NSImageView()
            view.imageScaling = .scaleProportionallyUpOrDown
            view.imageAlignment = .alignCenter
            return view
        }()

        weak var scrollView: NSScrollView?
        weak var image: NSImage?
        var lastCommandID: UUID?
        private var isFitMode = true
        private var liveMagnifyObserver: NSObjectProtocol?

        deinit {
            if let liveMagnifyObserver {
                NotificationCenter.default.removeObserver(liveMagnifyObserver)
            }
        }

        func connect(to scrollView: ViewportTrackingScrollView) {
            self.scrollView = scrollView
            scrollView.onViewportSizeChange = { [weak self] in
                guard self?.isFitMode == true else { return }
                self?.fit()
            }
            liveMagnifyObserver = NotificationCenter.default.addObserver(
                forName: NSScrollView.willStartLiveMagnifyNotification,
                object: scrollView,
                queue: .main
            ) { [weak self] _ in
                self?.isFitMode = false
            }
        }

        func fit() {
            guard let scrollView, imageView.frame.width > 0, imageView.frame.height > 0 else { return }
            // The clip view's bounds are expressed in magnified document coordinates.
            // Its frame remains the physical viewport size needed for a stable fit calculation.
            let viewport = scrollView.contentView.frame.size
            guard viewport.width > 0, viewport.height > 0 else { return }
            let plan = ImageViewportGeometry.magnificationPlan(
                imageWidth: Double(imageView.frame.width),
                imageHeight: Double(imageView.frame.height),
                viewportWidth: Double(viewport.width),
                viewportHeight: Double(viewport.height)
            )
            scrollView.minMagnification = CGFloat(plan.minimum)
            scrollView.maxMagnification = CGFloat(plan.maximum)
            isFitMode = true
            let center = NSPoint(x: imageView.frame.midX, y: imageView.frame.midY)
            scrollView.setMagnification(CGFloat(plan.fit), centeredAt: center)
        }

        func zoom(by factor: CGFloat) {
            guard let scrollView else { return }
            isFitMode = false
            let next = max(
                scrollView.minMagnification,
                min(scrollView.maxMagnification, scrollView.magnification * factor)
            )
            let visible = scrollView.documentVisibleRect
            scrollView.setMagnification(
                next,
                centeredAt: NSPoint(x: visible.midX, y: visible.midY)
            )
        }
    }
}
