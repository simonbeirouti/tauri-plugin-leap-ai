// swift-tools-version:5.9

import PackageDescription

let package = Package(
    name: "LeapSDKMobile",
    platforms: [
        .iOS(.v15),
    ],
    products: [
        .library(
            name: "LeapSDK",
            targets: ["LeapSDK", "LeapModelDownloader", "LeapSDKSupport"]
        ),
        .library(
            name: "LeapModelDownloader",
            targets: ["LeapModelDownloader"]
        ),
    ],
    targets: [
        .binaryTarget(
            name: "LeapSDK",
            url: "https://github.com/Liquid4All/leap-ios/releases/download/v0.9.3/LeapSDK.xcframework.zip",
            checksum: "141d791d3b803f36048a6873d474759733e486d3a46128ad2a7f5ac5fc23ace5"
        ),
        .binaryTarget(
            name: "InferenceEngine",
            url: "https://github.com/Liquid4All/leap-ios/releases/download/v0.9.3/inference_engine.xcframework.zip",
            checksum: "2124b68db5c113a3ff1411fc1e03f686682561731dbe512d9ecacb8eb047087a"
        ),
        .binaryTarget(
            name: "InferenceEngineExecutorchBackend",
            url: "https://github.com/Liquid4All/leap-ios/releases/download/v0.9.3/inference_engine_executorch_backend.xcframework.zip",
            checksum: "5e0d88385caed23e92813dcd54f06ca49616470ab23cfa18868b7dd5df0669d2"
        ),
        .binaryTarget(
            name: "InferenceEngineLlamaCppBackend",
            url: "https://github.com/Liquid4All/leap-ios/releases/download/v0.9.3/inference_engine_llamacpp_backend.xcframework.zip",
            checksum: "1060bf93cee4bc3d585576f280ca284927f97d6f5bfc0efa8fc6d6e17d075c93"
        ),
        .binaryTarget(
            name: "LeapModelDownloader",
            url: "https://github.com/Liquid4All/leap-ios/releases/download/v0.9.3/LeapModelDownloader.xcframework.zip",
            checksum: "cfb645ebf06c603351eb3ea911ca05764771665103d052366fe92e48a2f9d387"
        ),
        .target(
            name: "LeapSDKSupport",
            dependencies: [
                "InferenceEngine",
                "InferenceEngineExecutorchBackend",
                "InferenceEngineLlamaCppBackend",
            ],
            path: "Sources/LeapSDKSupport"
        ),
    ]
)
