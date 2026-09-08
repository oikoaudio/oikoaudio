# MTS-ESP client

Unmodified `Client/libMTSClient.h` and `Client/libMTSClient.cpp` from ODDSound/MTS-ESP revision `f214739b8832e7f297cb9970d0c0efbf783f1462`. Their permissive licence is included in each source file. Only the client shim is bundled; the tuning master and libMTS runtime remain external. With no master, Weft uses its normal tuning. The shim is vendored because the upstream client is distributed as C++ source, not a Rust package.
