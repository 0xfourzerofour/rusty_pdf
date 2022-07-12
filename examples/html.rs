use rusty_pdf::PDFSigningDocument;

fn main() {
    let merged_doc = PDFSigningDocument::generate_pdf_from_html(include_str!("data/test.html"));

    merged_doc.finished().save("generated_html.pdf").unwrap();
}
