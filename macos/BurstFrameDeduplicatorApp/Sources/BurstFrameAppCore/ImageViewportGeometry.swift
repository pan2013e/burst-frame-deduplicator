public struct ImageMagnificationPlan: Equatable, Sendable {
    public let fit: Double
    public let minimum: Double
    public let maximum: Double

    public init(fit: Double, minimum: Double, maximum: Double) {
        self.fit = fit
        self.minimum = minimum
        self.maximum = maximum
    }
}

public enum ImageViewportGeometry {
    public static func magnificationPlan(
        imageWidth: Double,
        imageHeight: Double,
        viewportWidth: Double,
        viewportHeight: Double,
        inset: Double = 24
    ) -> ImageMagnificationPlan {
        let values = [imageWidth, imageHeight, viewportWidth, viewportHeight, inset]
        guard values.allSatisfy(\.isFinite),
              imageWidth > 0,
              imageHeight > 0,
              viewportWidth > 0,
              viewportHeight > 0
        else {
            return ImageMagnificationPlan(fit: 1, minimum: 0.1, maximum: 16)
        }

        let usableWidth = max(1, viewportWidth - max(0, inset) * 2)
        let usableHeight = max(1, viewportHeight - max(0, inset) * 2)
        let fit = max(0.005, min(1, min(usableWidth / imageWidth, usableHeight / imageHeight)))
        return ImageMagnificationPlan(
            fit: fit,
            minimum: max(0.0025, fit * 0.1),
            maximum: max(8, fit * 16)
        )
    }

    public static func previewNeedsRefinement(
        availableWidth: Double,
        availableHeight: Double,
        magnification: Double,
        backingScale: Double,
        targetLongEdge: Double,
        upscaleTolerance: Double = 1.05,
        minimumResolutionGain: Double = 1.15
    ) -> Bool {
        let values = [
            availableWidth,
            availableHeight,
            magnification,
            backingScale,
            targetLongEdge,
            upscaleTolerance,
            minimumResolutionGain,
        ]
        guard values.allSatisfy(\.isFinite),
              availableWidth > 0,
              availableHeight > 0,
              magnification > 0,
              backingScale > 0,
              targetLongEdge > 0,
              upscaleTolerance >= 1,
              minimumResolutionGain > 1
        else { return false }

        let availableLongEdge = max(availableWidth, availableHeight)
        let resolutionGain = targetLongEdge / availableLongEdge
        let devicePixelScale = magnification * backingScale
        return resolutionGain >= minimumResolutionGain
            && devicePixelScale > upscaleTolerance
    }

    public static func replacementMagnification(
        oldMagnification: Double,
        oldImageWidth: Double,
        newImageWidth: Double,
        minimumMagnification: Double,
        maximumMagnification: Double
    ) -> Double {
        let values = [
            oldMagnification,
            oldImageWidth,
            newImageWidth,
            minimumMagnification,
            maximumMagnification,
        ]
        guard values.allSatisfy(\.isFinite),
              oldMagnification > 0,
              oldImageWidth > 0,
              newImageWidth > 0,
              minimumMagnification > 0,
              maximumMagnification >= minimumMagnification
        else { return 1 }

        let equivalent = oldMagnification * oldImageWidth / newImageWidth
        return max(minimumMagnification, min(maximumMagnification, equivalent))
    }
}
