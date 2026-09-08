# Supplemental upstream notices

`scripts/notices.py` collects license and notice files from each product’s locked normal/build dependencies, including conditional targets. Archives receive a generated `dependency-licenses.json`, the product license, third-party notes, font notices, and applicable native-library notices. Development-only dependencies are excluded.

`upstream.json` records supplemental texts for crates whose published packages omit them. The source URLs identify the exact upstream commits recorded by those crates; SHA-256 checksums detect changes. Dispatch and RealFFT declare MIT but supply no separate license text: their supplements retain the published manifest, package-author attribution, and standard MIT terms, explicitly identified as supplemental text. Do not represent these as upstream-supplied copyright notices.

To inspect or refresh a product inventory, run `python3 scripts/notices.py inton --output .scratch/inton-notices` from the workspace root. Packaging runs the same collector automatically and fails when a dependency has neither packaged notice files nor a reviewed supplement. The inventory spans conditional targets and may include packages not linked into a particular platform build.
