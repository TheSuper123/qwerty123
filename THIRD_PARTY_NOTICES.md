# Third-party software

MostServer uses isideload 0.5.0 (MIT, Nicholas Sharp / nab138) for Apple
authentication, provisioning and signing; idevice 0.1.68 (MIT, Jackson Coxson)
for Apple device services; and apple-codesign-quick (MIT, see its authors and
license) through isideload for Mach-O code signing. Most does not reuse iloader
branding, logos or UI assets and is not endorsed by these projects.

Other dependencies include Rust, Tokio, Axum, rustls, ring, SQLite/rusqlite,
rcgen, qrcode, ZIP and their transitive dependencies. Exact versions, registry
checksums and source identities are recorded in Cargo.lock.

Release ZIPs include `third-party-licenses/` containing dependency metadata and
license/notice files collected from the exact resolved source packages by
`scripts/collect_licenses.py`. This directory also identifies source download
locations for dependencies with source-availability requirements. Upstream
code is used without local modifications. The project's MIT license does not
replace dependency licenses.

The standalone distribution EXE embeds the same collected notices, accessible
through `MostServer.exe --licenses` and the local panel's Licenses link.

Apple drivers, Docker Desktop and the anisette-v3-server image are separate
optional prerequisites obtained from their respective authors. They are not
bundled into or relicensed by the MostServer executable.
