# SanicRS

A frontend for OpenSubsonic media servers (e.g. Navidrome) written in Rust/GTK4.

### Features
- Stream songs
- Lyrics
- Browse albums & artists
- DBus MPRIS integration
- Play music in the background
- Scrobble playback back to server
- ReplayGain support

### Server support

Should work with any server implementation following the [OpenSubsonic](https://opensubsonic.netlify.app/)
standard.

Tested on: [Navidrome](https://github.com/navidrome/navidrome/), [Gonic](https://github.com/sentriz/gonic).

### Building

Build a flatpak:

```shell
flatpak-builder --force-clean --user --install-deps-from=flathub --repo=repo --install builddir data/me.quartzy.sanicrs.yml
```