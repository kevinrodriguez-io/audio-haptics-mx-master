import AppKit
import Combine
import Foundation
import SwiftUI

@MainActor
final class AppState: ObservableObject {
    @Published var drumsEnabled: Bool = false
    @Published var sensitivity: Double = 0.65
    @Published var linkLabel: String = "none"
    @Published var hitsFired: UInt64 = 0
    @Published var optionsParked: Bool = false
    @Published var lastError: String?
    @Published var menuIcon: String = "music.note"
    @Published var isChangingMode: Bool = false
    @Published var presetName: String = "Classic"
    @Published var showConfig: Bool = false
    @Published var inputLevel: Double = 0
    @Published var waveform: [Float] = Array(repeating: 0, count: 64)
    @Published var audioFramesReceived: UInt64 = 0
    @Published var tapRunning: Bool = false

    private let audioTap = AudioTap()
    private var statusTimer: AnyCancellable?
    private var meterTimer: AnyCancellable?
    private var sensitivitySink: AnyCancellable?
    private var modeTask: Task<Void, Never>?
    private var modeEpoch: UInt64 = 0

    init() {
        if let cfg = loadConfigModel() {
            sensitivity = cfg.sensitivity
            presetName = cfg.name
        }
        md_set_sensitivity(Float(sensitivity))
        statusTimer = Timer.publish(every: 1.0, on: .main, in: .common)
            .autoconnect()
            .sink { [weak self] _ in
                self?.refreshStatus()
            }
        meterTimer = Timer.publish(every: 1.0 / 20.0, on: .main, in: .common)
            .autoconnect()
            .sink { [weak self] _ in
                self?.refreshMeter()
            }

        sensitivitySink = $sensitivity
            .dropFirst()
            .sink { value in
                md_set_sensitivity(Float(value))
            }
    }

    /// Prefer this over binding `$drumsEnabled` directly — avoids Combine re-entry loops.
    var drumsModeBinding: Binding<Bool> {
        Binding(
            get: { self.drumsEnabled },
            set: { self.requestDrumsMode($0) }
        )
    }

    func requestDrumsMode(_ enabled: Bool) {
        modeTask?.cancel()
        modeEpoch &+= 1
        let epoch = modeEpoch
        drumsEnabled = enabled
        isChangingMode = true
        modeTask = Task { [weak self] in
            guard let self else { return }
            await self.applyDrumsMode(enabled, epoch: epoch)
        }
    }

    func refreshMeter() {
        tapRunning = audioTap.isRunning
        audioFramesReceived = audioTap.framesReceived
        let peak = Double(audioTap.consumePeak())
        // Smooth the displayed level a bit.
        inputLevel = min(1.0, inputLevel * 0.65 + peak * 0.35)
        waveform = audioTap.snapshotWaveform()
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
        if let name = obj["preset_name"] as? String, !name.isEmpty {
            presetName = name
        }
        let running = obj["running"] as? Bool ?? false
        menuIcon = running ? "metronome.fill" : "music.note"

        // Keep a startup / permission error visible while stopped.
        if let engineErr = obj["last_error"] as? String, !engineErr.isEmpty {
            lastError = engineErr
        } else if running {
            lastError = nil
        }
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
        modeTask?.cancel()
        modeEpoch &+= 1
        drumsEnabled = false
        isChangingMode = false
        audioTap.stop()
        md_stop()
        _ = LogiMode.exitDrums()
        optionsParked = false
    }

    private func applyDrumsMode(_ enabled: Bool, epoch: UInt64) async {
        defer {
            if modeEpoch == epoch {
                isChangingMode = false
            }
        }

        if enabled {
            lastError = nil

            let hidStatus = HidAccess.requestIfNeeded()
            if Task.isCancelled || modeEpoch != epoch { return }
            if hidStatus != .granted {
                drumsEnabled = false
                optionsParked = optionsParked // leave as-is
                lastError = """
                Input Monitoring is \(hidStatus.rawValue) for MusicDrums.

                \(HidAccess.permissionHelp)
                """
                HidAccess.openSystemSettings()
                return
            }

            let park = LogiMode.enterDrums()
            if Task.isCancelled || modeEpoch != epoch { return }
            optionsParked = park.ok
            if !park.ok {
                lastError = "Could not park Options+: \(park.message)"
            }

            // Give Options+ time to release HID after kill + second pass in the script.
            try? await Task.sleep(nanoseconds: 900_000_000)
            if Task.isCancelled || modeEpoch != epoch { return }

            var startErr: String?
            var started = false
            for attempt in 1 ... 4 {
                if Task.isCancelled || modeEpoch != epoch { return }
                var err: UnsafeMutablePointer<CChar>?
                let startCode = md_start_with_error(&err)
                if startCode == 0 {
                    started = true
                    startErr = nil
                    break
                }
                if let err {
                    startErr = String(cString: err)
                    md_string_free(err)
                } else {
                    startErr = "Failed to start engine"
                }
                // Permission denials will not clear by retrying.
                if isHidPermissionError(startErr) {
                    break
                }
                try? await Task.sleep(nanoseconds: UInt64(350_000_000 * attempt))
            }

            if Task.isCancelled || modeEpoch != epoch { return }

            if !started {
                drumsEnabled = false
                // Keep Options+ parked so the next attempt matches CLI `disable` behavior.
                // Restoring here was causing: fail → Options+ back → HID fight → fail loop.
                optionsParked = true
                lastError = annotatedStartError(startErr)
                if isHidPermissionError(startErr) {
                    HidAccess.openSystemSettings()
                }
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
                md_set_sample_rate(Float(audioTap.sampleRate))
                if Task.isCancelled || modeEpoch != epoch {
                    audioTap.stop()
                    md_stop()
                    return
                }
                lastError = nil
                tapRunning = true
            } catch {
                lastError = "Audio tap: \(error.localizedDescription). Options+ still parked — turn Drums off to restore."
                md_stop()
                drumsEnabled = false
                optionsParked = true
                tapRunning = false
            }
        } else {
            audioTap.stop()
            md_stop()
            tapRunning = false
            inputLevel = 0
            waveform = Array(repeating: 0, count: 64)
            audioFramesReceived = 0
            let restore = LogiMode.exitDrums()
            if Task.isCancelled || modeEpoch != epoch { return }
            optionsParked = false
            if !restore.ok {
                lastError = restore.message
            } else if lastError == nil || !(lastError?.contains("0xE00002E2") ?? false) {
                lastError = nil
            }
        }
        refreshStatus()
    }

    private func isHidPermissionError(_ message: String?) -> Bool {
        guard let message else { return false }
        return message.contains("0xE00002E2")
            || message.localizedCaseInsensitiveContains("not permitted")
    }

    private func annotatedStartError(_ startErr: String?) -> String {
        let base = startErr ?? "Failed to start engine"
        if isHidPermissionError(base) {
            return """
            \(base)

            \(HidAccess.permissionHelp)

            Options+ is still parked. Turn Drums off when you want Options+ back.
            """
        }
        return """
        \(base)

        Options+ is still parked. Fix the link (wake mouse / Input Monitoring), then toggle on again — or toggle off to restore Options+.
        """
    }

    // MARK: - Config / presets (FFI)

    func loadConfigModel() -> DrumsConfigModel? {
        guard let cStr = md_config_json() else { return nil }
        defer { md_string_free(cStr) }
        let json = String(cString: cStr)
        guard let data = json.data(using: .utf8),
              let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any]
        else { return nil }
        return DrumsConfigModel.fromJSONObject(obj)
    }

    func listPresets() -> [PresetInfoModel] {
        guard let cStr = md_list_presets_json() else { return [] }
        defer { md_string_free(cStr) }
        let json = String(cString: cStr)
        guard let data = json.data(using: .utf8),
              let arr = try? JSONSerialization.jsonObject(with: data) as? [[String: Any]]
        else { return [] }
        return arr.compactMap { row in
            guard let id = row["id"] as? String, let name = row["name"] as? String else { return nil }
            return PresetInfoModel(
                id: id,
                name: name,
                description: row["description"] as? String ?? "",
                builtin: row["builtin"] as? Bool ?? false
            )
        }
    }

    func loadPreset(_ id: String) -> DrumsConfigModel? {
        var err: UnsafeMutablePointer<CChar>?
        let code = id.withCString { md_load_preset($0, &err) }
        if code != 0 {
            if let err {
                lastError = String(cString: err)
                md_string_free(err)
            }
            return nil
        }
        let cfg = loadConfigModel()
        if let cfg {
            sensitivity = cfg.sensitivity
            presetName = cfg.name
            md_set_sensitivity(Float(cfg.sensitivity))
        }
        return cfg
    }

    /// Returns error string on failure.
    func applyConfig(_ model: DrumsConfigModel) -> String? {
        do {
            let json = try model.toJSONString()
            var err: UnsafeMutablePointer<CChar>?
            let code = json.withCString { md_set_config_json($0, &err) }
            if code != 0 {
                if let err {
                    let msg = String(cString: err)
                    md_string_free(err)
                    return msg
                }
                return "Failed to apply config"
            }
            sensitivity = model.sensitivity
            presetName = model.name
            return nil
        } catch {
            return error.localizedDescription
        }
    }

    func saveCurrentPreset() -> String? {
        var err: UnsafeMutablePointer<CChar>?
        let code = md_save_preset(&err)
        if code != 0 {
            if let err {
                let msg = String(cString: err)
                md_string_free(err)
                return msg
            }
            return "Failed to save preset"
        }
        return nil
    }

    func importConfig(path: String) -> String? {
        var err: UnsafeMutablePointer<CChar>?
        let code = path.withCString { md_import_config($0, &err) }
        if code != 0 {
            if let err {
                let msg = String(cString: err)
                md_string_free(err)
                return msg
            }
            return "Failed to import"
        }
        return nil
    }
}
