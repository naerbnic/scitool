use sci_resources::{
    ResourceId, ResourceType,
    file::open_game_resources,
    types::{class_species::ClassSpeciesTable, script::load_script, selector_table::SelectorTable},
};
use sci_utils::buffer::Buffer;

fn main() {
    let arg = std::env::args().nth(1).unwrap();
    let path = std::path::Path::new(&arg);

    let resources = open_game_resources(path).unwrap();

    let species_table_resource = resources
        .get_resource(&ResourceId::new(ResourceType::Vocab, 996))
        .unwrap();

    let species_table =
        ClassSpeciesTable::load_from(species_table_resource.load_data().unwrap().narrow()).unwrap();

    println!("Species Table: {:#?}", species_table);

    let selector_table_resource = resources
        .get_resource(&ResourceId::new(ResourceType::Vocab, 997))
        .unwrap();

    let selector_table =
        SelectorTable::load_from(selector_table_resource.load_data().unwrap().narrow()).unwrap();

    for script_res in resources.resources_of_type(sci_resources::ResourceType::Script) {
        println!("Script Id: {:?}", script_res.id());
        let resource_id = sci_resources::ResourceId::new(
            sci_resources::ResourceType::Heap,
            script_res.id().resource_num(),
        );
        let heap_res = resources.get_resource(&resource_id).unwrap();
        let _loaded_script = load_script(
            &selector_table,
            &script_res.load_data().unwrap().narrow(),
            &heap_res.load_data().unwrap().narrow(),
        )
        .unwrap();
    }
}
