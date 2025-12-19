"""Python bindings for the mesh-thumbnail renderer."""

from ._core import (  # type: ignore[attr-defined]
    FORMAT_JPG,
    FORMAT_PNG,
    MeshThumbnailError,
    ThumbnailOptions,
    generate_thumbnail_bytes,
    generate_thumbnail_for_file,
)

__all__ = [
    "ThumbnailOptions",
    "generate_thumbnail_for_file",
    "generate_thumbnail_bytes",
    "MeshThumbnailError",
    "FORMAT_PNG",
    "FORMAT_JPG",
]
