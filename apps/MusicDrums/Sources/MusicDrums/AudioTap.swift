import AudioToolbox
import AVFoundation
import CoreAudio
import Foundation

/// Global system-audio Process Tap (macOS 14.4+). Pushes interleaved stereo f32 to a callback.
final class AudioTap {
    private var tapID: AudioObjectID = AudioObjectID(kAudioObjectUnknown)
    private var aggregateID: AudioObjectID = AudioObjectID(kAudioObjectUnknown)
    private var deviceProcID: AudioDeviceIOProcID?
    private var onFrames: (([Float]) -> Void)?
    private(set) var isRunning = false

    private var unknownID: AudioObjectID { AudioObjectID(kAudioObjectUnknown) }

    enum TapError: LocalizedError {
        case unsupported
        case createTap(OSStatus)
        case createAggregate(OSStatus)
        case start(OSStatus)
        case format

        var errorDescription: String? {
            switch self {
            case .unsupported:
                return "System audio tap requires macOS 14.4+"
            case .createTap(let s):
                return "AudioHardwareCreateProcessTap failed (\(s))"
            case .createAggregate(let s):
                return "AudioHardwareCreateAggregateDevice failed (\(s))"
            case .start(let s):
                return "AudioDeviceStart failed (\(s))"
            case .format:
                return "Could not read tap audio format"
            }
        }
    }

    func start(onFrames: @escaping ([Float]) -> Void) throws {
        stop()
        self.onFrames = onFrames

        if #unavailable(macOS 14.4) {
            throw TapError.unsupported
        }

        let description = CATapDescription(stereoGlobalTapButExcludeProcesses: [])
        description.name = "MusicDrums Tap"
        description.isPrivate = true

        var tap = AudioObjectID(kAudioObjectUnknown)
        let tapStatus = AudioHardwareCreateProcessTap(description, &tap)
        guard tapStatus == noErr else { throw TapError.createTap(tapStatus) }
        tapID = tap

        let tapUID = try tapUIDString(tapID)
        let outUID = try defaultOutputUID()

        let aggUID = "com.kevin.musicdrums.tap.\(UUID().uuidString)"
        let dict: [String: Any] = [
            kAudioAggregateDeviceNameKey: "MusicDrums Aggregate",
            kAudioAggregateDeviceUIDKey: aggUID,
            kAudioAggregateDeviceMainSubDeviceKey: outUID,
            kAudioAggregateDeviceIsPrivateKey: true,
            kAudioAggregateDeviceIsStackedKey: false,
            kAudioAggregateDeviceTapAutoStartKey: true,
            kAudioAggregateDeviceSubDeviceListKey: [
                [kAudioSubDeviceUIDKey: outUID],
            ],
            kAudioAggregateDeviceTapListKey: [
                [
                    kAudioSubTapUIDKey: tapUID,
                    kAudioSubTapDriftCompensationKey: true,
                ],
            ],
        ]

        var aggregate = AudioObjectID(kAudioObjectUnknown)
        let aggStatus = AudioHardwareCreateAggregateDevice(dict as CFDictionary, &aggregate)
        guard aggStatus == noErr else {
            destroyTap()
            throw TapError.createAggregate(aggStatus)
        }
        aggregateID = aggregate

        let callback: AudioDeviceIOProc = { _, _, inInputData, _, _, _, clientData in
            guard let clientData else { return noErr }
            let tap = Unmanaged<AudioTap>.fromOpaque(clientData).takeUnretainedValue()
            tap.handleInput(inInputData)
            return noErr
        }

        var procID: AudioDeviceIOProcID?
        let procStatus = AudioDeviceCreateIOProcID(
            aggregateID,
            callback,
            Unmanaged.passUnretained(self).toOpaque(),
            &procID
        )
        guard procStatus == noErr, let procID else {
            destroyAggregate()
            destroyTap()
            throw TapError.start(procStatus)
        }
        deviceProcID = procID

        let startStatus = AudioDeviceStart(aggregateID, procID)
        guard startStatus == noErr else {
            stop()
            throw TapError.start(startStatus)
        }
        isRunning = true
    }

    func stop() {
        if let procID = deviceProcID, aggregateID != unknownID {
            AudioDeviceStop(aggregateID, procID)
            AudioDeviceDestroyIOProcID(aggregateID, procID)
        }
        deviceProcID = nil
        destroyAggregate()
        destroyTap()
        onFrames = nil
        isRunning = false
    }

    deinit {
        stop()
    }

    private func handleInput(_ bufferList: UnsafePointer<AudioBufferList>?) {
        guard let bufferList, let onFrames else { return }
        let abl = UnsafeMutableAudioBufferListPointer(UnsafeMutablePointer(mutating: bufferList))
        guard let buf = abl.first, let data = buf.mData else { return }
        let count = Int(buf.mDataByteSize) / MemoryLayout<Float>.size
        let ptr = data.bindMemory(to: Float.self, capacity: count)
        let frames = Array(UnsafeBufferPointer(start: ptr, count: count))
        onFrames(frames)
    }

    private func destroyAggregate() {
        if aggregateID != unknownID {
            AudioHardwareDestroyAggregateDevice(aggregateID)
            aggregateID = unknownID
        }
    }

    private func destroyTap() {
        if tapID != unknownID {
            AudioHardwareDestroyProcessTap(tapID)
            tapID = unknownID
        }
    }

    private func tapUIDString(_ tap: AudioObjectID) throws -> String {
        var address = AudioObjectPropertyAddress(
            mSelector: kAudioTapPropertyUID,
            mScope: kAudioObjectPropertyScopeGlobal,
            mElement: kAudioObjectPropertyElementMain
        )
        var cfUID: Unmanaged<CFString>?
        var size = UInt32(MemoryLayout<CFString>.size)
        let status = withUnsafeMutablePointer(to: &cfUID) { ptr in
            AudioObjectGetPropertyData(tap, &address, 0, nil, &size, ptr)
        }
        guard status == noErr, let cfUID else { throw TapError.format }
        return cfUID.takeRetainedValue() as String
    }

    private func defaultOutputUID() throws -> String {
        var address = AudioObjectPropertyAddress(
            mSelector: kAudioHardwarePropertyDefaultOutputDevice,
            mScope: kAudioObjectPropertyScopeGlobal,
            mElement: kAudioObjectPropertyElementMain
        )
        var deviceID = AudioDeviceID()
        var size = UInt32(MemoryLayout<AudioDeviceID>.size)
        var status = AudioObjectGetPropertyData(
            AudioObjectID(kAudioObjectSystemObject),
            &address,
            0,
            nil,
            &size,
            &deviceID
        )
        guard status == noErr else { throw TapError.format }

        address.mSelector = kAudioDevicePropertyDeviceUID
        var cfUID: Unmanaged<CFString>?
        size = UInt32(MemoryLayout<CFString>.size)
        status = withUnsafeMutablePointer(to: &cfUID) { ptr in
            AudioObjectGetPropertyData(deviceID, &address, 0, nil, &size, ptr)
        }
        guard status == noErr, let cfUID else { throw TapError.format }
        return cfUID.takeRetainedValue() as String
    }
}
