# LibOrbisPkg PkgTool (operator tool for PS4 updates)

Upstream: https://github.com/OpenOrbis/LibOrbisPkg (LGPL-3.0), revision
`92a4e7f8e5914cb06b52ba3ae6b0675da438a051`. Apply
`marquee-net10-readonly-fullread.patch`, then:

    dotnet build PkgTool.Core/PkgTool.Core.csproj -c Release -o ../pkgtool
    dotnet ../pkgtool/PkgTool.Core.dll pkg_extract --passcode 00000000000000000000000000000000 in.pkg out/

The patch: targets .NET 10; maps the package read-only (the sandbox mounts it
read-only); and fixes compressed (PFSC) blocks, which were decoded with a single
`DeflateStream.Read`. Since .NET 6 that call may return part of a block, so
unpatched output is mostly zero-filled. Verified 2026-11-27: the patched build
matches ShadPKG byte for byte on all 58 DriveClub base files.
