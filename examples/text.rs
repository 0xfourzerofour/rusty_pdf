use lopdf::Document;
use std::fs;

use rusty_pdf::{Font, PDFSigningDocument};

#[derive(Debug)]
enum EventType {
    Sent,
    Completed,
    Signed,
}

#[derive(Debug)]
struct EventUser<'a> {
    email: &'a str,
    ip: &'a str,
    audit_id: &'a str,
}

#[derive(Debug)]
struct TableRow<'a> {
    event: EventType,
    time: &'a str,
    user: EventUser<'a>,
}

fn main() {
    let rows = vec![TableRow {
        event: EventType::Signed,
        time: "1234",
        user: EventUser {
            email: "hello@gmail.com",
            ip: "00.00.00.00",
            audit_id: "12345qwert",
        },
    }];

    for i in rows {
        print!("{:?}", i);
    }

    let doc_mem = fs::read("examples/data/pdf_example.pdf").unwrap_or(vec![]);

    let doc = Document::load_mem(&doc_mem).unwrap_or_default();

    let mut test_doc = PDFSigningDocument::new(doc);

    let page_id = *test_doc
        .get_document_ref()
        .get_pages()
        .get(&1)
        .unwrap_or(&(0, 0));

    test_doc
        .add_text_to_doc(
            "Hello from abstracted function",
            (0.0, 250.0),
            Font::Courier,
            27.0,
            page_id,
        )
        .unwrap();

    test_doc.finished().save("new_pdf_with_data.pdf").unwrap();
}
