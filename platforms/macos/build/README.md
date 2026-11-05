# macOS app bundle

This directory holds the scripts that build the macOS `.app`: `build.sh`, `app.sh`, the icon source and the icon
generator.

## Build

`make build_macos_app` builds `meshtown-macos` (the build with a tray) and wraps it into
`dist/macos-arm64/<APP_NAME>-<version>.app`, plus a distribution archive
`dist/macos-arm64/<APP_NAME>-macos-arm64-<version>.zip`. Inside the archive the package is `<APP_NAME>.app`.
`dist/` keeps the two newest versions.

## Bundle details

- `APP_NAME` is the package file name (`MeshTown` by default, giving `MeshTown.app` without a space), `APP_TITLE` is the
  name shown to users in `Info.plist` ("Mesh Town" by default), and `BUNDLE_ID` is `local.meshtown` by default.
  Override them with environment variables: `make build_macos_app APP_NAME=Foo`.
- The package is signed ad hoc (`codesign --force --deep --sign -`). The archive is made with
  `ditto --norsrc --noextattr`, so it has no `._*` files and one top-level directory. What a recipient sees on first
  launch is described in the Install section of the main README.
- There is no Dock icon (`LSUIElement=true`).
- The package's main file is the `meshtown-macos` binary copied in as `meshtown` (`CFBundleExecutable=meshtown`,
  no separate launcher). The process must match `CFBundleExecutable`, otherwise macOS 26 puts the menu bar icon off
  screen. Without arguments the binary starts `tray`.
- Logs are `~/Library/Application Support/meshtown/meshtown-tray.log` (the tray) and `meshtown.log` (the node, which the
  tray starts through `current_exe`); the previous file of each is kept as `.1`.
- If the node did not start, the tray menu's first line says why. If macOS hid the icon anyway, a window with the
  button "Open Settings" pops up 2 seconds after launch (Menu Bar → Allow in the Menu Bar).
