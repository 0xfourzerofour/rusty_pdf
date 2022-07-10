use imagesize::{blob_size, ImageSize};
use lopdf::Document;
use std::{fs, io::Cursor};

use rusty_pdf::{PDFSigningDocument, Rectangle};

fn main() {
    let doc_mem = fs::read("examples/data/pdf_example.pdf").unwrap_or(vec![]);

    let doc = Document::load_mem(&doc_mem).unwrap_or_default();

    let image_mem = fs::read("examples/data/signature_example.png").unwrap_or(vec![]);

    let dimensions = blob_size(&image_mem).unwrap_or(ImageSize {
        width: 0,
        height: 0,
    });

    let scaled_vec = Rectangle::scale_image_on_width(
        150.0,
        200.0,
        500.0,
        (dimensions.width as f64, dimensions.height as f64),
    );

    let file = Cursor::new(image_mem);
    let mut test_doc = PDFSigningDocument::new(doc);
    let object_id = test_doc.add_object_from_scaled_vec(scaled_vec);
    let page_id = *test_doc
        .get_document_ref()
        .get_pages()
        .get(&1)
        .unwrap_or(&(0, 0));

    test_doc
        .add_signature_to_form(file.clone(), "signature_1", page_id, object_id)
        .unwrap();

    test_doc.finished().save("new_pdf_with_data.pdf").unwrap();
}
