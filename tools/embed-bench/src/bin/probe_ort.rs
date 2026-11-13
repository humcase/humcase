//! Size probe with the model runtime and the tokenizer: the size it adds to an executable is
//! `probe_ort` minus `probe_none`.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dir = std::path::PathBuf::from(std::env::args().nth(1).unwrap_or_default());
    let tokenizer =
        tokenizers::Tokenizer::from_file(dir.join("tokenizer.json")).map_err(|e| e.to_string())?;
    let session = ort::session::Session::builder()?.commit_from_file(dir.join("model.onnx"))?;
    println!(
        "{} inputs, {:?}",
        session.inputs().len(),
        tokenizer.get_vocab_size(true)
    );
    Ok(())
}
