# Macroquad 0.4.16 atlas texture lifetime patch

This is the same crates.io version and feature set pinned by Cargo.lock before
vendoring. Miniquad remains the registry 0.4.11 dependency. Original MIT/Apache
licenses and package files are retained; source-provenance.json records every
original file hash, the original package checksum and both patched Rust files.
The complete change is in atlas-lifetime.patch.

Font atlas growth previously deleted the old GPU texture while glyph draws were
still queued. The graphics backend could also restore a cached deleted binding
during replacement allocation. Atlas replacement and drop now retire those raw
textures through the existing TexturesContext garbage collector. Context::end_frame
submits draws and commits the backend before that collector deletes them. Retired
textures live only through the current frame; unrelated texture bindings and
all glyph layout, filtering, shaders, animation and audio code are unchanged.

OpenGL resets the binding of a deleted bound texture to zero:
https://registry.khronos.org/OpenGL-Refpages/gl4/html/glDeleteTextures.xhtml

The GPU regression evidence under runs/client-texture-lifetime-validation compares
actual before/growth/after frames. Queued headers/glyphs are corrupted in the
unpatched case and RGBA-exact across the fixed adjacent frames. Replacement glyphs
remain exact. The old atlas is drawn before deletion; active texture count returns
to five after a transient peak of seven. The earlier Miniquad-only candidate left
one stale queued draw and is not part of this patch. Full cold/prewarmed images
are not asserted identical; the matched adjacent-frame regions are the controls.

Run python tools/check_macroquad_vendor.py to verify the source closure. The
standalone browser probes and the normal Windows/WASM client are verified
separately. Upstream path-dependency compiler warnings are preserved, rather than
editing unrelated library code. Historical packages and checkpoints stay immutable.
