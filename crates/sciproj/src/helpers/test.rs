//! Test helpers for tests in this crate.
#![cfg(test)]

use std::collections::BTreeMap;

pub(crate) fn make_map(
    items: impl IntoIterator<Item = (impl AsRef<str>, impl AsRef<str>)>,
) -> BTreeMap<String, String> {
    items
        .into_iter()
        .map(|(k, v)| (k.as_ref().to_string(), v.as_ref().to_string()))
        .collect()
}

macro_rules! assert_matches {
    ($expr: expr, $pat:pat) => {{
        let expr = $expr;
        assert!(
            matches!(expr, $pat),
            "Match failed:\n\texpected: {:?}\n\tactual: {:?}",
            stringify!($pat),
            expr
        );
    }};
}

macro_rules! from_json {
    ($($json:tt)*) => {
        serde_json::from_value(serde_json::json!( $($json)* )).unwrap()
    };
}

macro_rules! make_file {
    ($base_path:expr, $path:literal) => {
        // Create an empty file at the location.
        let path = $base_path.join($path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, "").unwrap();
    };

    ($base_path:expr, $path:literal => $contents:literal) => {
        // Create a file at the location with the given contents.
        let path = $base_path.join($path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, $contents).unwrap();
    };

    ($base_path:expr, $path:literal => {
        $($dir_contents:tt)*
    }) => {
        let new_base_path = $base_path.join($path);
        std::fs::create_dir_all(new_base_path.parent().unwrap()).unwrap();
        $crate::helpers::test::make_files!(new_base_path, $($dir_contents)*);
    };
}

macro_rules! make_files {
    ($root_path:expr, $($path:literal $(=> $contents:tt)?),* $(,)?) => {
        $(
            $crate::helpers::test::make_file!($root_path, $path $( => $contents)?);
        )*
    };
}

macro_rules! build_files {
    ($($path:literal $(=> $contents:tt)?),* $(,)?) => {{
        let root = tempfile::TempDir::new().unwrap();
        $(
            $crate::helpers::test::make_files!(root.path(), $path $( => $contents)?);
        )*
        root
    }};
}

pub(crate) use {assert_matches, from_json};
#[expect(unused_imports)]
pub(crate) use {build_files, make_file, make_files};

#[cfg(test)]
mod tests {
    #[test]
    fn test_build_files_for_single_file() {
        let root = build_files!("test.txt" => "test");
        assert_eq!(
            std::fs::read_to_string(root.path().join("test.txt")).unwrap(),
            "test"
        );
    }

    #[test]
    fn test_build_files_for_directory() {
        let root = build_files!("test_dir" => {
            "test.txt" => "test",
        });
        assert!(root.path().join("test_dir/test.txt").exists());
    }

    #[test]
    fn test_build_files_for_nested_directory() {
        let root = build_files!("d1/d2" => {
            "test.txt" => "test",
        });
        assert!(root.path().join("d1/d2/test.txt").exists());
    }

    #[test]
    fn test_build_files_for_empty_file() {
        let root = build_files!("empty.txt");
        assert!(
            std::fs::read_to_string(root.path().join("empty.txt"))
                .unwrap()
                .is_empty()
        );
    }
}
