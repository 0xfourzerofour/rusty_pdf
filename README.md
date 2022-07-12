# Rusty PDF

[![Crates.io](https://img.shields.io/crates/v/rusty_pdf)](https://crates.io/crates/rusty_pdf/)
[![API Docs](https://img.shields.io/badge/docs.rs-rusty_pdf-blue)](https://docs.rs/rusty_pdf/latest/)

This crate is a specialized crate that uses [`lopdf`][lopdf] to add images and 
text to selected pages.

This library only supports PNG images however I will be adding JPEG support.

The main aim of this library is to abstract some of the lower level PDF implementations in [`lopdf`][lopdf]
to an easy to use library for simple pdf manipulation tasks.

### This library was heavily inspired from the following project

[`pdf_signing`][pdf_signing]

I migrated to this repo as I have different a different end goal for the project, please check out his
project if it is something you are interested in!

## Current Features

  - Render html to pdf using headless chrome
  - Add text to pdf
  - Add png file to pdf
  - merge pdf files

## TODO

  - Allow importing of ttf files for font rendering
  - Add feature to allow jpeg images
  - Create solid documentation
  - introduce cbindgen and expose c api for easy integration with other languages
  
## License

The code in this project is licensed under the MIT or Apache 2.0 license.

All contributions, code and documentation, to this project will be similarly licensed.


[lopdf]: https://github.com/J-F-Liu/lopdf
[pdf_signing]: https://github.com/ralpha/pdf_signing
