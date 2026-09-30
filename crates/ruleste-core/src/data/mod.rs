//! Resource parsers. Each module decodes one of the original asset formats:
//! `.bin` maps, `.meta`/`.data` atlases, `.txt` dialogs and fonts. On top of
//! those, `audio`/`pack` parse the converted resource tree (manifests and
//! pack metadata), and `ogg` decodes PCM.

pub mod atlas;
pub mod audio;
pub mod binary_packer;
pub mod dialog;
pub mod font;
pub mod ogg;
pub mod pack;
pub mod reader;
pub mod spritebank;
