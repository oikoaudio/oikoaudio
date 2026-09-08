#include "Tunings.h"
#include <cmath>
#include <cstdio>
#include <cstring>
// Only plain C data crosses the boundary. All exceptions are caught here.
extern "C" int oiko_prepare_tuning(const char* scl, const char* kbm, double* hz,
                            double* period, int* count, int* ref, int* root,
                            double* original_ref, char* error, size_t error_len, double* degrees) noexcept {
    try {
        auto s = Tunings::parseSCLData(scl);
        if (s.count < 1 || s.count > 4096) throw std::runtime_error("Scale must have 1–4096 intervals");
        auto k = kbm ? Tunings::parseKBMData(kbm) : Tunings::startScaleOnAndTuneNoteTo(60,69,440.0);
        // Off-keyboard reference pitches occur in full-keyboard exports. Keep them
        // within tuning-library's 512-note table (-256..255), never clamp them.
        if (k.count < 0 || k.count > 4096 || k.firstMidi < 0 || k.lastMidi > 127 || k.firstMidi > k.lastMidi ||
            k.tuningConstantNote < -256 || k.tuningConstantNote > 255 || k.middleNote < 0 || k.middleNote > 127 ||
            !std::isfinite(k.tuningFrequency) || k.tuningFrequency <= 0)
            throw std::runtime_error("Invalid KBM range, reference note, root, or reference frequency");
        // Bound the library's expanded scale before its KBM unwrapping loop.
        if (k.octaveDegrees < 0 || k.octaveDegrees > 4096)
            throw std::runtime_error("KBM formal period degree must be between 0 and 4096");
        for (const auto degree : k.keys)
            if (degree < -1 || degree > 4096)
                throw std::runtime_error("KBM mapped degree exceeds the V1 limit of 4096");
        auto t = Tunings::Tuning(s,k);
        *period = std::exp2(s.tones.back().cents / 1200.0);
        if (!std::isfinite(*period) || *period <= 0) throw std::runtime_error("Invalid scale period");
        for (int n=0;n<128;++n) {
            if (!t.isMidiNoteMapped(n) || n<k.firstMidi || n>k.lastMidi)
                throw std::runtime_error("KBM has unmapped MIDI notes; V1 requires all 128 notes for morphing");
            hz[n] = t.frequencyForMidiNote(n)*440.0/k.tuningFrequency;
            if (!std::isfinite(hz[n]) || hz[n]<=0 || !std::isfinite(hz[n]*480.0/440.0*4.0))
                throw std::runtime_error("Scale produces an invalid or overflowing frequency");
        }
        for (int i=0;i<s.count;++i) degrees[i]=s.tones[i].cents;
        *count=s.count; *ref=k.tuningConstantNote; *root=k.middleNote; *original_ref=k.tuningFrequency;
        return 1;
    } catch(const std::exception& e) { std::snprintf(error,error_len,"%s",e.what()); }
      catch(...) { std::snprintf(error,error_len,"Unknown tuning-library error"); }
    return 0;
}
