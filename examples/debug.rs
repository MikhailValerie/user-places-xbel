fn main() -> Result<(), Box<dyn std::error::Error>> {

    let temp_file_path = tempfile::tempdir().path().join("test_file.txt");

    user_places_xbel::update_user_place(
        &temp_file_path,
        String::from("org.test"),
        String::from("test"),
        None,
    )?;

    let user_places = user_places_xbel::read_user_places()?;
    for bookmark in user_places.bookmarks {
        println!("{:?}", bookmark);
    }

    user_places_xbel::remove_user_place(&temp_file_path)?;

    Ok(())
}
