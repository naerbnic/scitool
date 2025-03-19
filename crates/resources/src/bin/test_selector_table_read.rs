use sci_resources::{
    ResourceId, ResourceType, file::open_game_resources, types::selector_table::SelectorTable,
};
use sci_utils::buffer::Buffer;

fn main() {
    let arg = std::env::args().nth(1).unwrap();
    let path = std::path::Path::new(&arg);

    let resources = open_game_resources(path).unwrap();

    let selector_table_resource = resources
        .get_resource(&ResourceId::new(ResourceType::Vocab, 997))
        .unwrap();

    let selector_table =
        SelectorTable::load_from(selector_table_resource.load_data().unwrap().narrow()).unwrap();
    println!("{:#?}", selector_table);
}
