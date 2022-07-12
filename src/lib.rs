mod error;
mod image_xobject;
mod pdf_object;
mod utils;

use headless_chrome::{Browser, LaunchOptionsBuilder, Tab};
use image_xobject::ImageXObject;
use lopdf::{content::Operation, dictionary, Bookmark, Document, Object, ObjectId};
use pdf_object::PdfObjectDeref;
use std::{
    collections::{BTreeMap, HashMap},
    io::Read,
    sync::Arc,
};

pub use error::Error;
pub use lopdf;
use utils::Server;

#[derive(Debug, Clone, Default)]
pub struct Rectangle {
    pub x1: f64,
    pub y1: f64,
    pub x2: f64,
    pub y2: f64,
}

impl Rectangle {
    pub fn scale_image_on_width(width: f64, x: f64, y: f64, dimensions: (f64, f64)) -> Self {
        let (dx, dy) = dimensions;
        let ratio = dy / dx;
        Self {
            x1: x,
            y1: y,
            x2: x + width,
            y2: y + width * ratio,
        }
    }
}

#[derive(Debug)]
pub enum Font {
    Helvetica,
    Courier,
    Fontier,
}

/// The whole PDF document. This struct only loads part of the document on demand.
#[derive(Debug, Clone)]
pub struct PDFSigningDocument {
    raw_document: Document,
    /// Link between the image name saved and the objectId of the image.
    /// This is used to reduce the amount of copies of the images in the pdf file.
    image_signature_object_id: HashMap<String, ObjectId>,
    //TODO add map of existing font and unsafe name within document
    // font_unsafe_name: HashMap<String, String>,
}

fn browser() -> Browser {
    Browser::new(
        LaunchOptionsBuilder::default()
            .headless(true)
            .build()
            .unwrap(),
    )
    .unwrap()
}

fn dumb_client(server: &Server) -> (Browser, Arc<Tab>) {
    let browser = browser();
    let tab = browser.wait_for_initial_tab().unwrap();
    tab.navigate_to(&format!("http://127.0.0.1:{}", server.port()))
        .unwrap();
    (browser, tab)
}

fn dumb_server(data: &'static str) -> (Server, Browser, Arc<Tab>) {
    let server = Server::with_dumb_html(data);
    let (browser, tab) = dumb_client(&server);
    (server, browser, tab)
}

impl PDFSigningDocument {
    pub fn new(raw_document: Document) -> Self {
        PDFSigningDocument {
            raw_document,
            image_signature_object_id: HashMap::new(),
        }
    }

    pub fn generate_pdf_from_html(html_content: &'static str) -> Self {
        let (_server, _browser, tab) = dumb_server(&html_content);

        let local_pdf = tab
            .wait_until_navigated()
            .unwrap()
            .print_to_pdf(None)
            .unwrap();

        let new_pdf = Document::load_mem(&local_pdf).unwrap();

        return PDFSigningDocument::new(new_pdf);
    }

    pub fn merge(documents: Vec<Document>) -> Result<Self, Error> {
        let mut max_id = 1;
        let mut pagenum = 1;

        let mut documents_pages = BTreeMap::new();
        let mut documents_objects = BTreeMap::new();
        let mut document = Document::with_version("1.5");

        for mut doc in documents {
            let mut first = false;
            doc.renumber_objects_with(max_id);

            max_id = doc.max_id + 1;

            documents_pages.extend(
                doc.get_pages()
                    .into_iter()
                    .map(|(_, object_id)| {
                        if !first {
                            let bookmark = Bookmark::new(
                                String::from(format!("Page_{}", pagenum)),
                                [0.0, 0.0, 1.0],
                                0,
                                object_id,
                            );
                            document.add_bookmark(bookmark, None);
                            first = true;
                            pagenum += 1;
                        }

                        (object_id, doc.get_object(object_id).unwrap().to_owned())
                    })
                    .collect::<BTreeMap<ObjectId, Object>>(),
            );
            documents_objects.extend(doc.objects);
        }

        // Catalog and Pages are mandatory
        let mut catalog_object: Option<(ObjectId, Object)> = None;
        let mut pages_object: Option<(ObjectId, Object)> = None;

        // Process all objects except "Page" type
        for (object_id, object) in documents_objects.iter() {
            // We have to ignore "Page" (as are processed later), "Outlines" and "Outline" objects
            // All other objects should be collected and inserted into the main Document
            match object.type_name().unwrap_or("") {
                "Catalog" => {
                    // Collect a first "Catalog" object and use it for the future "Pages"
                    catalog_object = Some((
                        if let Some((id, _)) = catalog_object {
                            id
                        } else {
                            *object_id
                        },
                        object.clone(),
                    ));
                }
                "Pages" => {
                    // Collect and update a first "Pages" object and use it for the future "Catalog"
                    // We have also to merge all dictionaries of the old and the new "Pages" object
                    if let Ok(dictionary) = object.as_dict() {
                        let mut dictionary = dictionary.clone();
                        if let Some((_, ref object)) = pages_object {
                            if let Ok(old_dictionary) = object.as_dict() {
                                dictionary.extend(old_dictionary);
                            }
                        }

                        pages_object = Some((
                            if let Some((id, _)) = pages_object {
                                id
                            } else {
                                *object_id
                            },
                            Object::Dictionary(dictionary),
                        ));
                    }
                }
                "Page" => {}     // Ignored, processed later and separately
                "Outlines" => {} // Ignored, not supported yet
                "Outline" => {}  // Ignored, not supported yet
                _ => {
                    document.objects.insert(*object_id, object.clone());
                }
            }
        }

        // If no "Pages" found abort
        if pages_object.is_none() {
            return Err(Error::Other("Pages root not found.".to_owned()));
        }

        // Iter over all "Page" and collect with the parent "Pages" created before
        for (object_id, object) in documents_pages.iter() {
            if let Ok(dictionary) = object.as_dict() {
                let mut dictionary = dictionary.clone();
                dictionary.set("Parent", pages_object.as_ref().unwrap().0);

                document
                    .objects
                    .insert(*object_id, Object::Dictionary(dictionary));
            }
        }

        // If no "Catalog" found abort
        if catalog_object.is_none() {
            return Err(Error::Other("Catalog root not found".to_owned()));
        }

        let catalog_object = catalog_object.unwrap();
        let pages_object = pages_object.unwrap();

        // Build a new "Pages" with updated fields
        if let Ok(dictionary) = pages_object.1.as_dict() {
            let mut dictionary = dictionary.clone();

            // Set new pages count
            dictionary.set("Count", documents_pages.len() as u32);

            // Set new "Kids" list (collected from documents pages) for "Pages"
            dictionary.set(
                "Kids",
                documents_pages
                    .into_iter()
                    .map(|(object_id, _)| Object::Reference(object_id))
                    .collect::<Vec<_>>(),
            );

            document
                .objects
                .insert(pages_object.0, Object::Dictionary(dictionary));
        }

        // Build a new "Catalog" with updated fields
        if let Ok(dictionary) = catalog_object.1.as_dict() {
            let mut dictionary = dictionary.clone();
            dictionary.set("Pages", pages_object.0);
            dictionary.remove(b"Outlines"); // Outlines not supported in merged PDFs

            document
                .objects
                .insert(catalog_object.0, Object::Dictionary(dictionary));
        }

        document.trailer.set("Root", catalog_object.0);

        // Update the max internal ID as wasn't updated before due to direct objects insertion
        document.max_id = document.objects.len() as u32;

        // Reorder all new Document objects
        document.renumber_objects();

        //Set any Bookmarks to the First child if they are not set to a page
        document.adjust_zero_pages();

        //Set all bookmarks to the PDF Object tree then set the Outlines to the Bookmark content map.
        if let Some(n) = document.build_outline() {
            if let Ok(x) = document.get_object_mut(catalog_object.0) {
                if let Object::Dictionary(ref mut dict) = x {
                    dict.set("Outlines", Object::Reference(n));
                }
            }
        }

        document.compress();

        Ok(Self {
            raw_document: document,
            image_signature_object_id: HashMap::new(),
        })
    }

    pub fn finished(self) -> Document {
        self.raw_document
    }

    pub fn get_document_ref(&self) -> &Document {
        &self.raw_document
    }

    pub fn add_object_from_scaled_vec(&mut self, rect: Rectangle) -> ObjectId {
        let object_id = self.raw_document.add_object(dictionary! {
            "Kids" => vec![Object::from(dictionary! {
                "Type" => "Annot",
                "Rect" => vec![rect.x1.into(),rect.y1.into(),rect.x2.into(),rect.y2.into()],
            })]
        });

        return object_id;
    }

    pub fn add_signature_to_form<R: Read>(
        &mut self,
        image_reader: R,
        image_name: &str,
        page_id: ObjectId,
        form_id: ObjectId,
    ) -> Result<ObjectId, Error> {
        let rect = Self::get_rectangle(form_id, &self.raw_document)?;

        let image_object_id_opt = self.image_signature_object_id.get(image_name).cloned();

        Ok(if let Some(image_object_id) = image_object_id_opt {
            // Image was already added so we can reuse it.
            self.add_image_to_page_only(image_object_id, image_name, page_id, rect)?
        } else {
            // Image was not added already so we need to add it in full
            let image_object_id = self.add_image(image_reader, image_name, page_id, rect)?;
            // Add signature to map
            self.image_signature_object_id
                .insert(image_name.to_owned(), image_object_id);
            image_object_id
        })
    }

    // add font if not used before and insert text in desired location
    pub fn add_text_to_doc(
        &mut self,
        text: &str,
        dimensions: (f64, f64),
        _font: Font,
        font_size: f64,
        page_id: ObjectId,
    ) -> Result<(), Error> {
        let mut page_content = self.raw_document.get_and_decode_page_content(page_id)?;

        let (x, y) = dimensions;

        let operations = vec![
            Operation::new("BT", vec![]),
            Operation::new("Tf", vec!["F1".into(), font_size.into()]),
            Operation::new("Td", vec![x.into(), y.into()]),
            Operation::new("Tj", vec![Object::string_literal(text)]),
            Operation::new("ET", vec![]),
        ];

        for i in operations {
            page_content.operations.push(i);
        }

        self.raw_document
            .change_page_content(page_id, page_content.encode()?)?;
        Ok(())
    }

    /// For an AcroForm find the rectangle on the page.
    fn get_rectangle(form_id: ObjectId, raw_doc: &Document) -> Result<Rectangle, Error> {
        let mut rect = None;
        // Get kids
        let form_dict = raw_doc.get_object(form_id)?.as_dict()?;
        let kids = if form_dict.has(b"Kids") {
            Some(form_dict.get(b"Kids")?.as_array()?)
        } else {
            None
        };

        if let Some(kids) = kids {
            for child in kids {
                let child_dict = child.deref(raw_doc)?.as_dict()?;
                // Child should be of `Type` `Annot` for Annotation.
                if child_dict.has(b"Rect") {
                    let child_rect = child_dict.get(b"Rect")?.as_array()?;
                    if child_rect.len() >= 4 {
                        // Found a reference, set as return value
                        rect = Some(Rectangle {
                            x1: child_rect[0].as_f64()?,
                            y1: child_rect[1].as_f64()?,
                            x2: child_rect[2].as_f64()?,
                            y2: child_rect[3].as_f64()?,
                        });
                    }
                }
            }
        }

        rect.ok_or_else(|| Error::Other("AcroForm: Rectangle not found.".to_owned()))
    }

    fn add_image<R: Read>(
        &mut self,
        image_reader: R,
        image_name: &str,
        page_id: ObjectId,
        rect: Rectangle,
    ) -> Result<ObjectId, Error> {
        // Load image
        let image_decoder = png::Decoder::new(image_reader);
        let (mut image_xobject, mask_xobject) = ImageXObject::try_from(image_decoder)?;
        // Add object to object list
        if let Some(mask_xobject) = mask_xobject {
            let mask_xobject_id = self.raw_document.add_object(mask_xobject);
            image_xobject.s_mask = Some(mask_xobject_id);
        }
        let image_xobject_id = self.raw_document.add_object(image_xobject);
        // Add object to xobject list on page (with new IR)
        // Because of the unique name this item will not be inserted more then once.
        self.raw_document
            .add_xobject(page_id, image_name, image_xobject_id)?;
        // Add xobject to layer (make visible)
        self.add_image_to_page_stream(image_name, page_id, rect)?;

        Ok(image_xobject_id)
    }

    fn add_image_to_page_only(
        &mut self,
        image_xobject_id: ObjectId,
        image_name: &str,
        page_id: ObjectId,
        rect: Rectangle,
    ) -> Result<ObjectId, Error> {
        // Add object to xobject list on page (with new IR)
        // Because of the unique name this item will not be inserted more then once.
        self.raw_document
            .add_xobject(page_id, image_name, image_xobject_id)?;
        // Add xobject to layer (make visible)
        self.add_image_to_page_stream(image_name, page_id, rect)?;

        Ok(image_xobject_id)
    }

    // The image must already be added to the object list!
    // Please use `add_image` instead.
    fn add_image_to_page_stream(
        &mut self,
        xobject_name: &str,
        page_id: ObjectId,
        rect: Rectangle,
    ) -> Result<(), Error> {
        let mut content = self.raw_document.get_and_decode_page_content(page_id)?;
        let position = (rect.x1, rect.y1);
        let size = (rect.x2 - rect.x1, rect.y2 - rect.y1);
        // The following lines use commands: see p643 (Table A.1) for more info
        // `q` = Save graphics state
        content.operations.push(Operation::new("q", vec![]));
        // `cm` = Concatenate matrix to current transformation matrix
        content.operations.push(Operation::new(
            "cm",
            vec![
                size.0.into(),
                0i32.into(),
                0i32.into(),
                size.1.into(),
                position.0.into(),
                position.1.into(),
            ],
        ));
        // `Do` = Invoke named XObject
        content.operations.push(Operation::new(
            "Do",
            vec![Object::Name(xobject_name.as_bytes().to_vec())],
        ));
        // `Q` = Restore graphics state
        content.operations.push(Operation::new("Q", vec![]));

        self.raw_document
            .change_page_content(page_id, content.encode()?)?;

        Ok(())
    }
}
