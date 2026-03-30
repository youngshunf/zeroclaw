use zeroclawlabs::sop::{SopExecutionMode, load_sops_from_directory};
use std::path::PathBuf;

fn main() {
    let path = PathBuf::from("/Users/mac/.huanxing/agents/media-creator-29/sops/");
    let res = load_sops_from_directory(&path, SopExecutionMode::Supervised);
    println!("{:#?}", res);
}
