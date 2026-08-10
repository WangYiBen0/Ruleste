use ruleste::data::binary_packer::MapBin;
fn main() {
    let path = std::env::args().nth(1).unwrap();
    let bin = MapBin::from_file(std::path::Path::new(&path)).unwrap();
    fn walk(el: &ruleste::data::binary_packer::Element, depth: usize) {
        let attrs: Vec<String> = el
            .attrs
            .iter()
            .map(|(k, v)| format!("{k}={:?}", v))
            .collect();
        let inner = if el.attr("innerText").is_some() {
            " <innerText>"
        } else {
            ""
        };
        println!("{}{} {:?}{}", "  ".repeat(depth), el.name, attrs, inner);
        for c in &el.children {
            walk(c, depth + 1);
        }
    }
    walk(&bin.root, 0);
}
