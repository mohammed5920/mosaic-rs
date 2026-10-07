mosaic-rs is a demo made with Rust & WGPU that implements hardware 2D virtual texturing to render image- & video-mosaics in real-time with significant (2000x+) levels of detail. 

This is a port of my previous [tile-based software renderer in Python.]("https://github.com/mohammed5920/pysaic")

![Demo](assets/readme/1.gif "Preview")

It supports generating mosaics from both images and videos, using any combination of the two.

## Performance
On a Ryzen 5 5600 / RX 5600 XT at 4K, using the entirety of [Parks & Recreation](https://en.wikipedia.org/wiki/Parks_and_Recreation) as the tileset:
- 1080p image/hybrid mosaics are rendered at 800-900fps
- 1080p videos are converted to mosaics at roughly 300fps

[Read more](https://madebymohammed.com/mosaic-rs/)