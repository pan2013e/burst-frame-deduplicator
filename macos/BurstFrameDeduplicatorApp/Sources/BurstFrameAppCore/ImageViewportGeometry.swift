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
}
