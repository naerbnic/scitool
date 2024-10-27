use sci_resources::{file::open_game_resources, types::script::load_script};
use sci_utils::buffer::Buffer;

fn main() {
    let arg = std::env::args().nth(1).unwrap();
    let path = std::path::Path::new(&arg);

    let resources = open_game_resources(path).unwrap();

    for script_res in resources.resources_of_type(sci_resources::ResourceType::Script) {
        println!("Script Id: {:?}", script_res.id());
        let resource_id = sci_resources::ResourceId::new(
            sci_resources::ResourceType::Heap,
            script_res.id().resource_num(),
        );
        let heap_res = resources.get_resource(&resource_id).unwrap();
        let _loaded_script = load_script(
            &script_res.load_data().unwrap().narrow(),
            &heap_res.load_data().unwrap().narrow(),
        )
        .unwrap();
    }
}
