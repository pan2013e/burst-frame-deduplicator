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

private let refinedPreviewLongEdge = 4_096

private struct ImageResolutionDemand {
    let availablePixelSize: NSSize
    let magnification: CGFloat
    let backingScale: CGFloat
}

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
    @State private var refining = false
    @State private var rawRefinementAvailable = false
    @State private var refinementRequestAssetID: String?
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
                    ZoomableImageCanvas(
                        image: image,
                        command: canvasCommand,
                        onResolutionDemand: handleResolutionDemand
                    )
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
        .task(id: refinementRequestAssetID) {
            guard let assetID = refinementRequestAssetID else { return }
            await refineCurrentImage(assetID: assetID)
        }
        .onDisappear { refinementRequestAssetID = nil }
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
                if model.isCounterpartMoved(asset) {
                    Label(locale.text("counterpartMoved"), systemImage: "rectangle.2.swap")
                        .font(.callout)
                        .foregroundStyle(.teal)
                }
                ZStack {
                    if refining {
                        ProgressView()
                            .controlSize(.small)
                    }
                }
                .frame(width: 16, height: 16)
                .help(locale.text("loadingPreview"))
                .accessibilityHidden(!refining)
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
        refining = false
        rawRefinementAvailable = false
        refinementRequestAssetID = nil
        loadError = nil
        image = nil
        let initialResult = await Task.detached(priority: .userInitiated) {
            Result {
                let preview = try bridge.preparePreview(
                    runDirectory: runDirectory,
                    assetID: asset.id,
                    maxLongEdge: UInt32(refinedPreviewLongEdge),
                    generateIfMissing: false
                )
                let loaded = try loadDownsampledImage(
                    at: preview.path,
                    maxPixelSize: refinedPreviewLongEdge,
                    preferEmbeddedPreview: asset.representative.kind == "raw" && !preview.generated
                )
                return (loaded, preview.generated)
            }
        }.value
        guard !Task.isCancelled, model.viewerAssetID == asset.id else { return }
        switch initialResult {
        case .success(let (loaded, generated)):
            image = loaded
            canvasCommand = CanvasCommand(action: .fit)
            rawRefinementAvailable = asset.representative.kind == "raw" && !generated
        case .failure(let error):
            loadError = error.localizedDescription
            loading = false
            return
        }
        loading = false
    }

    private func handleResolutionDemand(_ demand: ImageResolutionDemand) {
        guard let asset = currentAsset,
              asset.representative.kind == "raw",
              rawRefinementAvailable
        else {
            if !refining, refinementRequestAssetID != nil {
                refinementRequestAssetID = nil
            }
            return
        }
        let needsRefinement = ImageViewportGeometry.previewNeedsRefinement(
            availableWidth: Double(demand.availablePixelSize.width),
            availableHeight: Double(demand.availablePixelSize.height),
            magnification: Double(demand.magnification),
            backingScale: Double(demand.backingScale),
            targetLongEdge: Double(refinedPreviewLongEdge)
        )
        if needsRefinement {
            refinementRequestAssetID = asset.id
        } else if !refining, refinementRequestAssetID == asset.id {
            refinementRequestAssetID = nil
        }
    }

    private func refineCurrentImage(assetID: String) async {
        do {
            try await Task.sleep(for: .milliseconds(350))
        } catch {
            return
        }
        guard !Task.isCancelled,
              model.viewerAssetID == assetID,
              rawRefinementAvailable,
              let runDirectory = model.payload?.runDir
        else { return }
        refining = true
        let refinedResult = await Task.detached(priority: .utility) {
            Result {
                let preview = try bridge.preparePreview(
                    runDirectory: runDirectory,
                    assetID: assetID,
                    maxLongEdge: UInt32(refinedPreviewLongEdge),
                    generateIfMissing: true
                )
                return try loadDownsampledImage(
                    at: preview.path,
                    maxPixelSize: refinedPreviewLongEdge,
                    preferEmbeddedPreview: false
                )
            }
        }.value
        guard !Task.isCancelled, model.viewerAssetID == assetID else { return }
        refining = false
        rawRefinementAvailable = false
        refinementRequestAssetID = nil
        if case .success(let refinedImage) = refinedResult {
            image = refinedImage
        }
    }
}

private func loadDownsampledImage(
    at path: String,
    maxPixelSize: Int,
    preferEmbeddedPreview: Bool
) throws -> NSImage {
    let strategy = preferEmbeddedPreview ? "embedded" : "rendered"
    let cacheKey = "\(path)#\(maxPixelSize)#\(strategy)" as NSString
    if let cached = previewImageCache.object(forKey: cacheKey) { return cached }
    let url = URL(fileURLWithPath: path) as CFURL
    guard let source = CGImageSourceCreateWithURL(url, nil) else {
        throw CocoaError(.fileReadCorruptFile, userInfo: [NSFilePathErrorKey: path])
    }
    var options: [CFString: Any] = [
        kCGImageSourceCreateThumbnailWithTransform: true,
        kCGImageSourceThumbnailMaxPixelSize: maxPixelSize,
        kCGImageSourceShouldCacheImmediately: true,
    ]
    options[preferEmbeddedPreview
        ? kCGImageSourceCreateThumbnailFromImageIfAbsent
        : kCGImageSourceCreateThumbnailFromImageAlways] = true
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
    let onResolutionDemand: (ImageResolutionDemand) -> Void

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
        coordinator.onResolutionDemand = onResolutionDemand
        if coordinator.image !== image {
            coordinator.replaceImage(image, pixelSize: imagePixelSize(image))
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
        var onResolutionDemand: ((ImageResolutionDemand) -> Void)?
        private var isFitMode = true
        private var liveMagnifyObservers: [NSObjectProtocol] = []

        deinit {
            for observer in liveMagnifyObservers {
                NotificationCenter.default.removeObserver(observer)
            }
        }

        func connect(to scrollView: ViewportTrackingScrollView) {
            self.scrollView = scrollView
            scrollView.onViewportSizeChange = { [weak self] in
                guard let self else { return }
                if self.isFitMode {
                    self.fit()
                } else {
                    self.reportResolutionDemand()
                }
            }
            liveMagnifyObservers.append(NotificationCenter.default.addObserver(
                forName: NSScrollView.willStartLiveMagnifyNotification,
                object: scrollView,
                queue: .main
            ) { [weak self] _ in
                self?.isFitMode = false
            })
            liveMagnifyObservers.append(NotificationCenter.default.addObserver(
                forName: NSScrollView.didEndLiveMagnifyNotification,
                object: scrollView,
                queue: .main
            ) { [weak self] _ in
                self?.reportResolutionDemand()
            })
        }

        func replaceImage(_ newImage: NSImage, pixelSize: NSSize) {
            guard let scrollView else { return }
            let oldSize = imageView.frame.size
            let hadImage = image != nil && oldSize.width > 0 && oldSize.height > 0
            let preserveViewport = hadImage && !isFitMode
            let oldMagnification = scrollView.magnification
            let oldVisible = scrollView.documentVisibleRect
            let normalizedCenter = NSPoint(
                x: oldSize.width > 0 ? oldVisible.midX / oldSize.width : 0.5,
                y: oldSize.height > 0 ? oldVisible.midY / oldSize.height : 0.5
            )

            image = newImage
            guard hadImage else {
                imageView.image = newImage
                imageView.frame = NSRect(origin: .zero, size: pixelSize)
                imageView.needsDisplay = true
                DispatchQueue.main.async { [weak self] in self?.fit() }
                return
            }

            NSAnimationContext.runAnimationGroup { context in
                context.duration = 0
                context.allowsImplicitAnimation = false
                imageView.image = newImage
                imageView.frame = NSRect(origin: .zero, size: pixelSize)
                imageView.needsDisplay = true
                guard preserveViewport else {
                    fit(reportDemand: false)
                    return
                }
                let plan = self.magnificationPlan()
                scrollView.minMagnification = CGFloat(plan.minimum)
                scrollView.maxMagnification = CGFloat(plan.maximum)
                let magnification = ImageViewportGeometry.replacementMagnification(
                    oldMagnification: Double(oldMagnification),
                    oldImageWidth: Double(oldSize.width),
                    newImageWidth: Double(pixelSize.width),
                    minimumMagnification: plan.minimum,
                    maximumMagnification: plan.maximum
                )
                let center = NSPoint(
                    x: normalizedCenter.x * pixelSize.width,
                    y: normalizedCenter.y * pixelSize.height
                )
                scrollView.setMagnification(CGFloat(magnification), centeredAt: center)
                isFitMode = false
            }
        }

        func fit(reportDemand: Bool = true) {
            guard let scrollView, imageView.frame.width > 0, imageView.frame.height > 0 else { return }
            // The clip view's bounds are expressed in magnified document coordinates.
            // Its frame remains the physical viewport size needed for a stable fit calculation.
            let viewport = scrollView.contentView.frame.size
            guard viewport.width > 0, viewport.height > 0 else { return }
            let plan = magnificationPlan()
            scrollView.minMagnification = CGFloat(plan.minimum)
            scrollView.maxMagnification = CGFloat(plan.maximum)
            isFitMode = true
            let center = NSPoint(x: imageView.frame.midX, y: imageView.frame.midY)
            scrollView.setMagnification(CGFloat(plan.fit), centeredAt: center)
            if reportDemand { reportResolutionDemand() }
        }

        private func magnificationPlan() -> ImageMagnificationPlan {
            guard let scrollView else {
                return ImageMagnificationPlan(fit: 1, minimum: 0.1, maximum: 16)
            }
            let viewport = scrollView.contentView.frame.size
            return ImageViewportGeometry.magnificationPlan(
                imageWidth: Double(imageView.frame.width),
                imageHeight: Double(imageView.frame.height),
                viewportWidth: Double(viewport.width),
                viewportHeight: Double(viewport.height)
            )
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
            reportResolutionDemand()
        }

        private func reportResolutionDemand() {
            guard let scrollView,
                  imageView.frame.width > 0,
                  imageView.frame.height > 0
            else { return }
            let backingScale = scrollView.window?.backingScaleFactor
                ?? NSScreen.main?.backingScaleFactor
                ?? 1
            onResolutionDemand?(ImageResolutionDemand(
                availablePixelSize: imageView.frame.size,
                magnification: scrollView.magnification,
                backingScale: backingScale
            ))
        }
    }
}
