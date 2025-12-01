# pdf-converter

A cross-platform command-line tool to convert PDF files to PNG or SVG.

## Usage

```
Convert PDF files to PNG or SVG

Usage: pdf-converter [OPTIONS] <FORMAT> <INPUT> [OUTPUT]

Arguments:
  <FORMAT>  Output format [possible values: png, svg]
  <INPUT>   Input PDF file
  [OUTPUT]  Output directory [default: .]

Options:
  -q, --quiet            Suppress informational logging (only errors printed)
  -p, --page <PAGE>      Choose pages to convert. You can provide multiple page
                         numbers separated by commas
  -s, --scale <SCALE>    Scale factor applied to outputs [default: 1.0]
      --prefix <PREFIX>  Prefix for output files. If omitted, inferred from the
                         input name
  -h, --help             Print help
  -V, --version          Print version
```

## Examples

Render all pages to PNG files and write them to the `output` directory:

```
pdf-converter png my.pdf output
```

Convert pages 1 and 4 to SVG at 2.5× scale and write them to the current work directory:

```
pdf-converter -p 1,4 -s 2.5 svg my.pdf
```

Write output files with a custom prefix:

```
pdf-converter svg --prefix report my.pdf
```
