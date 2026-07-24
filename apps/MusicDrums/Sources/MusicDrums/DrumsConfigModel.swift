import Foundation

struct PresetInfoModel: Identifiable, Equatable {
    var id: String
    var name: String
    var description: String
    var builtin: Bool
}

struct DrumsConfigModel: Equatable {
    var schema: Int = 1
    var id: String
    var name: String
    var description: String
    var sensitivity: Double
    var detector: DetectorModel
    var mapper: MapperModel
    var engine: EngineModel

    struct DetectorModel: Equatable {
        var mode: String
        var cooldownMs: Double
        var historyLen: Int
        var fluxBase: Double
        var fluxMeanMul: Double
        var lowWeight: Double
        var midWeight: Double
        var highWeight: Double
        var lpHz: Double
        var bassRatioMin: Double
        var floorMulBase: Double
        var floorMulSensSpan: Double
        var cooldownSensSpanMs: Double
    }

    struct MapperModel: Equatable {
        var minIntervalMs: Double
        var scaleIntervalWithSensitivity: Bool
        var intensityFloor: Int
        var intensityCeil: Int
        var dropHats: Bool
        var strengthScaleBase: Double
        var strengthScaleSens: Double
        var minStrengthBase: Double
        var minStrengthSensSpan: Double
        var kickStrongAt: Double
        var kickTickAt: Double
        var useCompoundKick: Bool
        var pulseDynamics: Bool
    }

    struct EngineModel: Equatable {
        var drainFrames: Int
        var idleSleepMs: UInt64
        var intensityUpdateEveryHits: Int
        var intensityDeltaThreshold: Int
        var baseIntensity: Int
    }

    static func classic() -> DrumsConfigModel {
        DrumsConfigModel(
            id: "classic",
            name: "Classic",
            description: "Original multi-band onset detector (pre-house tuning).",
            sensitivity: 0.65,
            detector: DetectorModel(
                mode: "multiband",
                cooldownMs: 80,
                historyLen: 48,
                fluxBase: 0.002,
                fluxMeanMul: 2.5,
                lowWeight: 1.6,
                midWeight: 1.1,
                highWeight: 0.7,
                lpHz: 140,
                bassRatioMin: 0.55,
                floorMulBase: 3.2,
                floorMulSensSpan: 3.5,
                cooldownSensSpanMs: 0
            ),
            mapper: MapperModel(
                minIntervalMs: 70,
                scaleIntervalWithSensitivity: false,
                intensityFloor: 60,
                intensityCeil: 100,
                dropHats: false,
                strengthScaleBase: 0.5,
                strengthScaleSens: 1.0,
                minStrengthBase: 0.08,
                minStrengthSensSpan: 0,
                kickStrongAt: 0.55,
                kickTickAt: 0.25,
                useCompoundKick: true,
                pulseDynamics: false
            ),
            engine: EngineModel(
                drainFrames: 512,
                idleSleepMs: 1,
                intensityUpdateEveryHits: 1,
                intensityDeltaThreshold: 0,
                baseIntensity: 75
            )
        )
    }

    func toJSONString() throws -> String {
        let obj: [String: Any] = [
            "schema": schema,
            "id": id,
            "name": name,
            "description": description,
            "sensitivity": sensitivity,
            "detector": [
                "mode": detector.mode,
                "cooldown_ms": detector.cooldownMs,
                "history_len": detector.historyLen,
                "flux_base": detector.fluxBase,
                "flux_mean_mul": detector.fluxMeanMul,
                "low_weight": detector.lowWeight,
                "mid_weight": detector.midWeight,
                "high_weight": detector.highWeight,
                "lp_hz": detector.lpHz,
                "bass_ratio_min": detector.bassRatioMin,
                "floor_mul_base": detector.floorMulBase,
                "floor_mul_sens_span": detector.floorMulSensSpan,
                "cooldown_sens_span_ms": detector.cooldownSensSpanMs,
            ],
            "mapper": [
                "min_interval_ms": mapper.minIntervalMs,
                "scale_interval_with_sensitivity": mapper.scaleIntervalWithSensitivity,
                "intensity_floor": mapper.intensityFloor,
                "intensity_ceil": mapper.intensityCeil,
                "drop_hats": mapper.dropHats,
                "strength_scale_base": mapper.strengthScaleBase,
                "strength_scale_sens": mapper.strengthScaleSens,
                "min_strength_base": mapper.minStrengthBase,
                "min_strength_sens_span": mapper.minStrengthSensSpan,
                "kick_strong_at": mapper.kickStrongAt,
                "kick_tick_at": mapper.kickTickAt,
                "use_compound_kick": mapper.useCompoundKick,
                "pulse_dynamics": mapper.pulseDynamics,
            ],
            "engine": [
                "drain_frames": engine.drainFrames,
                "idle_sleep_ms": engine.idleSleepMs,
                "intensity_update_every_hits": engine.intensityUpdateEveryHits,
                "intensity_delta_threshold": engine.intensityDeltaThreshold,
                "base_intensity": engine.baseIntensity,
            ],
        ]
        let data = try JSONSerialization.data(withJSONObject: obj, options: [.prettyPrinted, .sortedKeys])
        return String(data: data, encoding: .utf8) ?? "{}"
    }

    static func fromJSONObject(_ obj: [String: Any]) -> DrumsConfigModel? {
        guard let id = obj["id"] as? String,
              let name = obj["name"] as? String,
              let det = obj["detector"] as? [String: Any],
              let map = obj["mapper"] as? [String: Any],
              let eng = obj["engine"] as? [String: Any]
        else { return nil }

        func d(_ dict: [String: Any], _ key: String, _ fallback: Double) -> Double {
            if let n = dict[key] as? NSNumber { return n.doubleValue }
            return fallback
        }
        func i(_ dict: [String: Any], _ key: String, _ fallback: Int) -> Int {
            if let n = dict[key] as? NSNumber { return n.intValue }
            return fallback
        }
        func b(_ dict: [String: Any], _ key: String, _ fallback: Bool) -> Bool {
            if let v = dict[key] as? Bool { return v }
            return fallback
        }

        let classic = DrumsConfigModel.classic()
        return DrumsConfigModel(
            schema: i(obj, "schema", 1),
            id: id,
            name: name,
            description: obj["description"] as? String ?? "",
            sensitivity: d(obj, "sensitivity", 0.65),
            detector: DetectorModel(
                mode: det["mode"] as? String ?? "multiband",
                cooldownMs: d(det, "cooldown_ms", classic.detector.cooldownMs),
                historyLen: i(det, "history_len", classic.detector.historyLen),
                fluxBase: d(det, "flux_base", classic.detector.fluxBase),
                fluxMeanMul: d(det, "flux_mean_mul", classic.detector.fluxMeanMul),
                lowWeight: d(det, "low_weight", classic.detector.lowWeight),
                midWeight: d(det, "mid_weight", classic.detector.midWeight),
                highWeight: d(det, "high_weight", classic.detector.highWeight),
                lpHz: d(det, "lp_hz", classic.detector.lpHz),
                bassRatioMin: d(det, "bass_ratio_min", classic.detector.bassRatioMin),
                floorMulBase: d(det, "floor_mul_base", classic.detector.floorMulBase),
                floorMulSensSpan: d(det, "floor_mul_sens_span", classic.detector.floorMulSensSpan),
                cooldownSensSpanMs: d(det, "cooldown_sens_span_ms", classic.detector.cooldownSensSpanMs)
            ),
            mapper: MapperModel(
                minIntervalMs: d(map, "min_interval_ms", classic.mapper.minIntervalMs),
                scaleIntervalWithSensitivity: b(map, "scale_interval_with_sensitivity", false),
                intensityFloor: i(map, "intensity_floor", classic.mapper.intensityFloor),
                intensityCeil: i(map, "intensity_ceil", classic.mapper.intensityCeil),
                dropHats: b(map, "drop_hats", false),
                strengthScaleBase: d(map, "strength_scale_base", classic.mapper.strengthScaleBase),
                strengthScaleSens: d(map, "strength_scale_sens", classic.mapper.strengthScaleSens),
                minStrengthBase: d(map, "min_strength_base", classic.mapper.minStrengthBase),
                minStrengthSensSpan: d(map, "min_strength_sens_span", classic.mapper.minStrengthSensSpan),
                kickStrongAt: d(map, "kick_strong_at", classic.mapper.kickStrongAt),
                kickTickAt: d(map, "kick_tick_at", classic.mapper.kickTickAt),
                useCompoundKick: b(map, "use_compound_kick", true),
                pulseDynamics: b(map, "pulse_dynamics", false)
            ),
            engine: EngineModel(
                drainFrames: i(eng, "drain_frames", classic.engine.drainFrames),
                idleSleepMs: UInt64(i(eng, "idle_sleep_ms", Int(classic.engine.idleSleepMs))),
                intensityUpdateEveryHits: i(eng, "intensity_update_every_hits", classic.engine.intensityUpdateEveryHits),
                intensityDeltaThreshold: i(eng, "intensity_delta_threshold", classic.engine.intensityDeltaThreshold),
                baseIntensity: i(eng, "base_intensity", classic.engine.baseIntensity)
            )
        )
    }
}
