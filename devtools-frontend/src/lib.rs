use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "$OUT_DIR/devtools_dist"]
pub struct Assets;
