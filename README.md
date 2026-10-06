# vid2gba

Turns videos into Game Boy Advance cartridges. Drop in a video, get a `.gba` file, play it on an emulator or a real GBA with a flash cart. I tested it on an EverDrive X5.

This garbage was made with AI. Use it, don't use it. I don't care.

## What it does

- Squeezes your video into 32 MiB. It tries the best quality first and steps down until it fits.
- Trim with a start and end time.
- Several videos on one cart get a DVD style menu with thumbnails.
- Subtitles from `.srt` or `.vtt` files, or the ones already in the video.
- Pick Emulator or Real GBA. Real GBA brightens the picture because the original screen is dark.

## Controls on the cart

- **START** or **A**: pause
- **Left / Right**: seek 5 seconds
- **L**: fill the screen
- **R**: original aspect
- **SELECT**: menu (subtitles, picture, restart, back to the main menu)

## Command line

Needs `ffmpeg` and `ffprobe` on your PATH.

```
cargo run --release -p vid2gba -- movie.mp4
cargo run --release -p vid2gba -- movie.mp4 --start 1:30 --end 4:00 --target gba
cargo run --release -p vid2gba -- one.mp4 two.mp4 three.mp4 --title "Road Trip"
```

Run it with `--help` for the rest.

## Player

The GBA side is a small C player that gets baked into every cart.

```
make -C player
```

Needs `arm-none-eabi-gcc`.

## License

MIT. The player is zlib so the carts you make are yours with no strings attached.
