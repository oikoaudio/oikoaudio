cmake_minimum_required(VERSION 3.21)

project(oiko-weft-auv2 VERSION 0.1.1 LANGUAGES C CXX OBJC OBJCXX)

if(NOT APPLE)
    message(FATAL_ERROR "The Oiko Weft AUv2 wrapper can only be built on macOS")
endif()

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_POSITION_INDEPENDENT_CODE ON)

set(WEFT_CLAP_BUNDLE "" CACHE PATH "Path to the universal Oiko Weft.clap bundle")
if(NOT IS_DIRECTORY "${WEFT_CLAP_BUNDLE}")
    message(FATAL_ERROR "WEFT_CLAP_BUNDLE must point to the Oiko Weft.clap bundle")
endif()

# Keep the wrapper revision fixed so the public AU identity and translation layer
# do not change underneath a Weft build.
include(FetchContent)
set(CLAP_WRAPPER_DOWNLOAD_DEPENDENCIES TRUE CACHE BOOL "" FORCE)
set(CLAP_WRAPPER_BUILD_TESTS FALSE CACHE BOOL "" FORCE)
set(CLAP_WRAPPER_COPY_AFTER_BUILD FALSE CACHE BOOL "" FORCE)
FetchContent_Declare(
    clap_wrapper
    GIT_REPOSITORY https://github.com/free-audio/clap-wrapper.git
    GIT_TAG 1cca996e96f29ab2be7ae9f8cfe532bbc92e1dd6 # v0.16.0
)
FetchContent_MakeAvailable(clap_wrapper)

# The pinned wrapper does not map Apple's Music Effect (`aumf`) type to its
# effect implementation and otherwise silently generates music-device behavior.
set(CLAP_WRAPPER_SOURCE "${clap_wrapper_SOURCE_DIR}")
include("${CMAKE_CURRENT_SOURCE_DIR}/patch-clap-wrapper.cmake")

add_library(oiko_weft_auv2 MODULE)
target_add_auv2_wrapper(
    TARGET oiko_weft_auv2
    OUTPUT_NAME "Oiko Weft"
    BUNDLE_IDENTIFIER "com.oikoaudio.weft.auv2"
    BUNDLE_VERSION "${PROJECT_VERSION}"
    PREFER_CMAKE_AUV2_CONFIGURATION TRUE
    MANUFACTURER_NAME "Oiko Audio"
    MANUFACTURER_CODE "Oiko"
    SUBTYPE_CODE "Weft"
    INSTRUMENT_TYPE "aumf"
    MACOS_EMBEDDED_CLAP_LOCATION "${WEFT_CLAP_BUNDLE}"
)

# With Makefile generators, clap-wrapper's PRE_BUILD plist copy can run before
# CMake writes its default bundle plist, leaving a valid-looking component with
# no AudioComponents registration. Make the generated AU plist the final write.
set(WEFT_AUV2_INFO_PLIST
    "${CMAKE_CURRENT_BINARY_DIR}/oiko_weft_auv2-build-helper-output/auv2_Info.plist"
)
set(WEFT_AUV2_EMBEDDED_CLAP
    "$<TARGET_BUNDLE_DIR:oiko_weft_auv2>/Contents/PlugIns/Oiko Weft.clap"
)
add_custom_command(
    TARGET oiko_weft_auv2
    POST_BUILD
    COMMAND "${CMAKE_COMMAND}" -E copy
        "${WEFT_AUV2_INFO_PLIST}"
        "$<TARGET_BUNDLE_DIR:oiko_weft_auv2>/Contents/Info.plist"
    COMMENT "Installing Oiko Weft Audio Unit registration metadata"
)
add_custom_target(
    oiko_weft_auv2_finalize
    COMMAND "${CMAKE_COMMAND}" -E rm -rf
        "${WEFT_AUV2_EMBEDDED_CLAP}"
    COMMAND "${CMAKE_COMMAND}" -E copy_directory
        "${WEFT_CLAP_BUNDLE}"
        "${WEFT_AUV2_EMBEDDED_CLAP}"
    COMMAND "${CMAKE_COMMAND}" -E copy
        "${WEFT_AUV2_INFO_PLIST}"
        "$<TARGET_BUNDLE_DIR:oiko_weft_auv2>/Contents/Info.plist"
    DEPENDS oiko_weft_auv2
    COMMENT "Refreshing embedded CLAP and finalizing Oiko Weft Audio Unit"
)
