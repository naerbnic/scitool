use sci_resources::{
    file::open_game_resources,
    types::{heap::Heap, script::Script},
};

fn main() {
    let arg = std::env::args().nth(1).unwrap();
    let path = std::path::Path::new(&arg);

    let resources = open_game_resources(path).unwrap();
    for res in resources.resources_of_type(sci_resources::ResourceType::Heap) {
        println!("Heap Id: {:?}", res.id());
        Heap::from_block(res.load_data().unwrap()).unwrap();
    }

    for res in resources.resources_of_type(sci_resources::ResourceType::Script) {
        println!("Script Id: {:?}", res.id());
        let _script = Script::from_block(res.load_data().unwrap()).unwrap();
    }
}
