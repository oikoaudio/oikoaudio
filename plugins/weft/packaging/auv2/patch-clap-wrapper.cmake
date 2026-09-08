# clap-wrapper 0.15.1 (and current main at the time of writing) advertises an
# `aumf` component correctly in Info.plist but falls through to its music-device
# behavior when generating the C++ entry point. Treat a Music Effect like the
# existing audio-effect path while retaining the wrapper's MIDI entry points.
set(helper "${CLAP_WRAPPER_SOURCE}/src/detail/auv2/build-helper/build-helper.cpp")
file(READ "${helper}" contents)

set(before "else if (u.type == \"aufx\")")
set(after "else if (u.type == \"aufx\" || u.type == \"aumf\")")
string(FIND "${contents}" "${after}" already_patched)
if(already_patched EQUAL -1)
    string(FIND "${contents}" "${before}" patch_location)
    if(patch_location EQUAL -1)
        message(FATAL_ERROR "The pinned clap-wrapper AU type generator changed; update the local aumf patch")
    endif()
    string(REPLACE "${before}" "${after}" contents "${contents}")
    file(WRITE "${helper}" "${contents}")
endif()
