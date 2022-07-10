use lopdf::Document;
use std::fs;

use rusty_pdf::PDFSigningDocument;

fn main() {
    let doc = fs::read("examples/data/pdf_example.pdf").unwrap_or(vec![]);

    let doc1 = Document::load_mem(&doc).unwrap_or_default();
    let doc2 = Document::load_mem(&doc).unwrap_or_default();
    let doc3 = Document::load_mem(&doc).unwrap_or_default();

    let merged_doc = PDFSigningDocument::merge(vec![doc1, doc2, doc3]).unwrap();

    merged_doc.finished().save("merged.pdf").unwrap();
}
