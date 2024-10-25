use sci_resources::{file::open_game_resources, types::heap::Heap};

fn main() {
    let arg = std::env::args().nth(1).unwrap();
    let path = std::path::Path::new(&arg);

    let resources = open_game_resources(path).unwrap();
    for res in resources.resources_of_type(sci_resources::ResourceType::Heap) {
        println!("Id: {:?}", res.id());
        Heap::from_block(res.load_data().unwrap()).unwrap();
    }
}
