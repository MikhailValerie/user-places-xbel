fn main() -> Result<(), Box<dyn std::error::Error>> {
    let user_places = user_places_xbel::parse_file()?;

    for bookmark in user_places.bookmarks {
        println!("{:#?}", bookmark);
    }

    Ok(())
}
