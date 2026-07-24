import AppKit
import Combine
import Foundation

@MainActor
final class AppState: ObservableObject {
    @Published var drumsEnabled: Bool = false
    @Published var sensitivity: Double = 0.65
    @Published var linkLabel: String = "none"
    @Published var hitsFired: UInt64 = 0
    @Published var optionsParked: Bool = false
    @Published var lastError: String?
    @Published var menuIcon: String = "music.note"

    private let audioTap = AudioTap()
    private var statusTimer: AnyCancellable?
    private var applyingMode = false
    private var sensitivitySink: AnyCancellable?
    private var drumsSink: AnyCancellable?

    init() {
        md_set_sensitivity(Float(sensitivity))
        statusTimer = Timer.publish(every: 1.0, on: .main, in: .common)
            .autoconnect()
            .sink { [weak self] _ in
                self?.refreshStatus()
            }

        drumsSink = $drumsEnabled
            .dropFirst()
            .removeDuplicates()
            .sink { [weak self] enabled in
                guard let self, !self.applyingMode else { return }
                Task { await self.setDrumsMode(enabled) }
            }

        sensitivitySink = $sensitivity
            .dropFirst()
            .sink { value in
                md_set_sensitivity(Float(value))
            }
    }

    func refreshStatus() {
        guard let cStr = md_status_json() else { return }
        defer { md_string_free(cStr) }
        let json = String(cString: cStr)
        guard let data = json.data(using: .utf8),
              let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any]
        else { return }

        let link = (obj["link"] as? String) ?? "none"
        linkLabel = link
        hitsFired = (obj["hits_fired"] as? NSNumber)?.uint64Value
            ?? UInt64(obj["hits_fired"] as? Int ?? 0)
        lastError = obj["last_error"] as? String
        let running = obj["running"] as? Bool ?? false
        menuIcon = running ? "metronome.fill" : "music.note"
    }

    func testPulse() {
        var err: UnsafeMutablePointer<CChar>?
        let code = md_test_pulse_with_error(80, &err)
        if code != 0 {
            if let err {
                lastError = String(cString: err)
                md_string_free(err)
            } else {
                lastError = "Test pulse failed"
            }
        } else {
            lastError = nil
        }
        refreshStatus()
    }

    func shutdown() {
        applyingMode = true
        drumsEnabled = false
        applyingMode = false
        audioTap.stop()
        md_stop()
        _ = LogiMode.exitDrums()
    }

    private func setDrumsMode(_ enabled: Bool) async {
        applyingMode = true
        defer { applyingMode = false }

        if enabled {
            lastError = nil
            let park = LogiMode.enterDrums()
            optionsParked = park.success
            if !park.success {
                lastError = park.message
            }

            try? await Task.sleep(nanoseconds: 500_000_000)

            var err: UnsafeMutablePointer<CChar>?
            let startCode = md_start_with_error(&err)
            if startCode != 0 {
                if let err {
                    lastError = String(cString: err)
                    md_string_free(err)
                } else {
                    lastError = "Failed to start engine"
                }
                drumsEnabled = false
                _ = LogiMode.exitDrums()
                optionsParked = false
                refreshStatus()
                return
            }

            do {
                try audioTap.start { frames in
                    frames.withUnsafeBufferPointer { buf in
                        if let base = buf.baseAddress {
                            md_push_audio_frames(base, UInt32(buf.count))
                        }
                    }
                }
            } catch {
                lastError = "Audio tap: \(error.localizedDescription)"
                md_stop()
                drumsEnabled = false
                _ = LogiMode.exitDrums()
                optionsParked = false
            }
        } else {
            audioTap.stop()
            md_stop()
            let restore = LogiMode.exitDrums()
            optionsParked = false
            if !restore.success {
                lastError = restore.message
            }
        }
        refreshStatus()
    }
}
