import Foundation

enum LogiMode {
    struct Result {
        var ok: Bool
        var message: String

        var success: Bool { ok }
    }

    static func enterDrums() -> Result {
        run(argument: "enter-drums")
    }

    static func exitDrums() -> Result {
        run(argument: "exit-drums")
    }

    static func status() -> Result {
        run(argument: "status")
    }

    private static func scriptURL() -> URL? {
        if let bundled = Bundle.main.url(forResource: "logi-mode", withExtension: "sh") {
            return bundled
        }
        let cwd = URL(fileURLWithPath: FileManager.default.currentDirectoryPath)
        let candidates = [
            cwd.appendingPathComponent("Scripts/logi-mode.sh"),
            cwd.appendingPathComponent("../Scripts/logi-mode.sh"),
            URL(fileURLWithPath: #filePath)
                .deletingLastPathComponent()
                .deletingLastPathComponent()
                .deletingLastPathComponent()
                .deletingLastPathComponent()
                .appendingPathComponent("Scripts/logi-mode.sh"),
        ]
        return candidates.first {
            FileManager.default.isExecutableFile(atPath: $0.path)
                || FileManager.default.fileExists(atPath: $0.path)
        }
    }

    private static func run(argument: String) -> Result {
        guard let script = scriptURL() else {
            return Result(ok: false, message: "logi-mode.sh not found")
        }
        let proc = Process()
        proc.executableURL = URL(fileURLWithPath: "/bin/bash")
        proc.arguments = [script.path, argument]
        let out = Pipe()
        let err = Pipe()
        proc.standardOutput = out
        proc.standardError = err
        do {
            try proc.run()
            proc.waitUntilExit()
            let stdout = String(data: out.fileHandleForReading.readDataToEndOfFile(), encoding: .utf8) ?? ""
            let stderr = String(data: err.fileHandleForReading.readDataToEndOfFile(), encoding: .utf8) ?? ""
            let combined = (stdout + stderr).trimmingCharacters(in: .whitespacesAndNewlines)
            return Result(ok: proc.terminationStatus == 0, message: combined)
        } catch {
            return Result(ok: false, message: error.localizedDescription)
        }
    }
}
