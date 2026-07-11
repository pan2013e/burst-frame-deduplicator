import BurstFrameAppCore
import Testing

@Test
func rustBridgeDefaultOptions() throws {
    let options = try RustBridge().defaultOptions()
    #expect(options.previewSize == 1280)
    #expect(options.refineSize == 2048)
    #expect(options.acceleration == "auto")
}

@Test @MainActor
func externalLocaleCatalogLoadsBothLanguages() {
    let catalog = LocaleCatalog()
    #expect(catalog.loadError == nil)
    catalog.code = "en"
    #expect(catalog.text("keep") == "Keep")
    catalog.code = "zh-CN"
    #expect(catalog.text("keep") == "保留")
}

@Test
func tutorialProgressRecordsCompletionAndSkip() {
    var encodedRecord: String?
    var legacyCompleted = false
    let finishedAt = "2026-07-12T00:00:00Z"
    let store = TutorialProgressStore(
        readRecord: { encodedRecord },
        writeRecord: { encodedRecord = $0 },
        legacyCompleted: { legacyCompleted },
        markLegacyComplete: { legacyCompleted = true },
        now: { finishedAt }
    )
    #expect(!store.hasFinished())

    store.record(.skipped)
    #expect(store.hasFinished())
    #expect(store.currentRecord() == TutorialProgressRecord(outcome: .skipped, finishedAt: finishedAt))

    store.record(.completed)
    #expect(store.currentRecord()?.outcome == .completed)
}

@Test
func tutorialProgressMigratesLegacyCompletion() {
    var encodedRecord: String?
    var legacyCompleted = true
    let store = TutorialProgressStore(
        readRecord: { encodedRecord },
        writeRecord: { encodedRecord = $0 },
        legacyCompleted: { legacyCompleted },
        markLegacyComplete: { legacyCompleted = true },
        now: { "2026-07-12T00:00:00Z" }
    )
    #expect(store.hasFinished())
    #expect(store.currentRecord()?.outcome == .completed)
    #expect(encodedRecord != nil)
}

@Test
func imageMagnificationFitsInsidePhysicalViewport() {
    let plan = ImageViewportGeometry.magnificationPlan(
        imageWidth: 6_000,
        imageHeight: 4_000,
        viewportWidth: 1_200,
        viewportHeight: 800
    )
    #expect(abs(plan.fit - 0.188) < 0.000_001)
    #expect(plan.minimum < plan.fit)
    #expect(plan.maximum > plan.fit)
}

@Test
func imageMagnificationDoesNotUpscaleSmallImages() {
    let plan = ImageViewportGeometry.magnificationPlan(
        imageWidth: 400,
        imageHeight: 300,
        viewportWidth: 1_200,
        viewportHeight: 900
    )
    #expect(plan.fit == 1)
}
