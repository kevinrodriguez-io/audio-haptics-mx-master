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
    private(set) var sampleRate: Double = 48_000
    private(set) var channelCount: Int = 2

    /// Latest peak abs sample (0…1+), updated on the audio thread.
    private let statsLock = NSLock()
    private var peakValue: Float = 0
    private var framesValue: UInt64 = 0
    private var waveRing: [Float] = Array(repeating: 0, count: 64)
    private var waveWrite = 0

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

    var latestPeak: Float {
        statsLock.lock(); defer { statsLock.unlock() }
        return peakValue
    }

    var framesReceived: UInt64 {
        statsLock.lock(); defer { statsLock.unlock() }
        return framesValue
    }

    func consumePeak() -> Float {
        statsLock.lock(); defer { statsLock.unlock() }
        let v = peakValue
        peakValue = 0
        return v
    }

    func snapshotWaveform() -> [Float] {
        statsLock.lock(); defer { statsLock.unlock() }
        return waveRing
    }

    func start(onFrames: @escaping ([Float]) -> Void) throws {
        stop()
        self.onFrames = onFrames
        statsLock.lock()
        framesValue = 0
        peakValue = 0
        statsLock.unlock()

        if #unavailable(macOS 14.4) {
            throw TapError.unsupported
        }

        // Global mixdown of all processes. Empty exclude list = everything.
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

        if let asbd = streamFormat(aggregateID) {
            sampleRate = asbd.mSampleRate > 0 ? asbd.mSampleRate : 48_000
            channelCount = Int(asbd.mChannelsPerFrame > 0 ? asbd.mChannelsPerFrame : 2)
        }

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
        guard !abl.isEmpty else { return }

        var interleaved: [Float] = []
        interleaved.reserveCapacity(4096)

        if abl.count >= 2, abl[0].mNumberChannels == 1, abl[1].mNumberChannels == 1 {
            // Non-interleaved stereo: buffer0 = L, buffer1 = R
            let lCount = Int(abl[0].mDataByteSize) / MemoryLayout<Float>.size
            let rCount = Int(abl[1].mDataByteSize) / MemoryLayout<Float>.size
            let n = min(lCount, rCount)
            guard let lPtr = abl[0].mData?.bindMemory(to: Float.self, capacity: lCount),
                  let rPtr = abl[1].mData?.bindMemory(to: Float.self, capacity: rCount)
            else { return }
            interleaved.reserveCapacity(n * 2)
            var peak: Float = 0
            for i in 0 ..< n {
                let l = lPtr[i]
                let r = rPtr[i]
                peak = max(peak, abs(l), abs(r))
                interleaved.append(l)
                interleaved.append(r)
            }
            finishBuffer(interleaved, peak: peak, onFrames: onFrames)
            return
        }

        guard let buf = abl.first, let data = buf.mData else { return }
        let count = Int(buf.mDataByteSize) / MemoryLayout<Float>.size
        guard count > 0 else { return }
        let ptr = data.bindMemory(to: Float.self, capacity: count)
        let channels = max(1, Int(buf.mNumberChannels))

        var peak: Float = 0
        if channels == 1 {
            interleaved.reserveCapacity(count * 2)
            for i in 0 ..< count {
                let s = ptr[i]
                peak = max(peak, abs(s))
                interleaved.append(s)
                interleaved.append(s)
            }
        } else {
            // Interleaved (or first buffer already packed).
            interleaved = Array(UnsafeBufferPointer(start: ptr, count: count))
            for s in interleaved {
                peak = max(peak, abs(s))
            }
        }
        finishBuffer(interleaved, peak: peak, onFrames: onFrames)
    }

    private func finishBuffer(_ frames: [Float], peak: Float, onFrames: ([Float]) -> Void) {
        guard !frames.isEmpty else { return }
        statsLock.lock()
        peakValue = max(peakValue, peak)
        framesValue &+= UInt64(frames.count)
        let step = max(1, frames.count / 8)
        var i = 0
        while i < frames.count {
            waveRing[waveWrite % waveRing.count] = abs(frames[i])
            waveWrite += 1
            i += step
        }
        statsLock.unlock()
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

    private func streamFormat(_ device: AudioObjectID) -> AudioStreamBasicDescription? {
        var address = AudioObjectPropertyAddress(
            mSelector: kAudioDevicePropertyStreamFormat,
            mScope: kAudioDevicePropertyScopeInput,
            mElement: kAudioObjectPropertyElementMain
        )
        var asbd = AudioStreamBasicDescription()
        var size = UInt32(MemoryLayout<AudioStreamBasicDescription>.size)
        let status = AudioObjectGetPropertyData(device, &address, 0, nil, &size, &asbd)
        return status == noErr ? asbd : nil
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
