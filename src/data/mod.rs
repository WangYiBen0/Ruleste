//! Resource parsers. Each module decodes one of the original asset formats:
//! `.bin` maps, `.meta`/`.data` atlases, `.txt` dialogs and fonts.

pub mod atlas;
pub mod binary_packer;
pub mod reader;
pub mod spritebank;
