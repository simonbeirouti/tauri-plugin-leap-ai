import XCTest
@testable import tauri_plugin_leap_ai

final class ExamplePluginTests: XCTestCase {
    func testPluginConstructs() throws {
        let plugin = ExamplePlugin()
        XCTAssertNotNil(plugin)
    }
}
