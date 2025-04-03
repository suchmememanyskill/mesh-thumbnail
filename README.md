# Mesh Thumbnail Generator

Supported formats:
- 3mf
- stl 
- obj
- stl (zipped)
- obj (zipped)

Supported output types:
- png
- jpg

```
3D file thumbnail generator

Usage: mesh-thumbnail.exe [OPTIONS] <FILES>...

Arguments:
  <FILES>...  Input files (at least one required)

Options:
      --rotatex <ROTATEX>       Rotation around the X-axis [default: 0]
      --rotatey <ROTATEY>       Rotation around the Y-axis [default: 0]
      --outdir <OUTDIR>         Output directory (default: current folder) [default: .]
      --width <WIDTH>           Image width [default: 512]
      --height <HEIGHT>         Image height [default: 512]
      --format <FORMAT>         Output image format [default: png] [possible values: jpg, png]
      --color <COLOR>           Background color in hex format (default: Grey) [default: DDDDDD]
      --overwrite               Overwrite existing output files
      --fallback-3mf-thumbnail  Fallback on thumbnail inside 3mf files
      --prefer-3mf-thumbnail    Prefer 3mf thumbnail over 3mf model
  -h, --help                    Print help
  -V, --version                 Print version
```

### Example

![Example](./example.png)