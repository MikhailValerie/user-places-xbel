// Copyright 2026 Andrew Moran <developer@moran.io>
// SPDX-License-Identifier: MPL-2.0

//! Parse the `~/.local/share/user-places.xbel` file
//!
//! ```
//! fn main() -> Result<(), Box<dyn std::error::Error>> {
//!
//!     let temp_file_path = tempfile::tempdir().path().join("test_file.txt");
//!
//!     user_places_xbel::update_user_place(
//!         &temp_file_path,
//!         String::from("org.test"),
//!         String::from("test"),
//!         None,
//!     )?;
//!
//!     let user_places = user_places_xbel::read_user_places()?;
//!     for bookmark in user_places.bookmarks {
//!         println!("{:?}", bookmark);
//!     }
//!
//!     user_places_xbel::remove_user_place(&temp_file_path)?;
//!
//!     Ok(())
//! }
//! ```

use chrono::{DateTime, SecondsFormat, Utc};
use custom_writer::custom_write;
use quick_xml::DeError;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    fs::{self},
    path::{Path, PathBuf},
    time::SystemTime,
    io::Write
};
use url::Url;
use atomicwrites::{AtomicFile,AllowOverwrite};

mod custom_writer;

// The normal user-places.xbel file name location
const USER_PLACES_FILE_SUBPATH: &str = ".local/share/user-places.xbel";

/// Stores places bookmarked by the desktop user
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename = "xbel", rename_all = "kebab-case")]
pub struct UserPlaces {
    #[serde(rename = "@xmlns:bookmark")]
    pub xmlns_bookmark: String,
    #[serde(rename = "@xmlns:mime")]
    pub xmlns_mime: String,

    /// Files that have been recently used.
    #[serde(rename = "bookmark", default)]
    pub bookmarks: Vec<Bookmark>,
}

/// A file bookmarke by the desktop user
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct Bookmark {
    /// The location of the file.
    #[serde(rename = "@href")]
    pub href: String,
    /// When the file was added to the list.
    #[serde(rename = "@added")]
    pub added: Option<String>,
    /// When the file was last modified.
    #[serde(rename = "@modified")]
    pub modified: Option<String>,
    /// When the file was last visited.
    #[serde(rename = "@visited")]
    pub visited: Option<String>,
    /// Additional metadata and applications related to the bookmark.
    #[serde(rename = "info")]
    pub info: Option<Info>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct Info {
    /// Metadata about the bookmark.
    #[serde(rename = "metadata")]
    pub metadata: Metadata,
}

/// Metadata containing MIME type and application info.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct Metadata {
    /// The owner of the metadata.
    #[serde(rename = "@owner")]
    pub owner: String,

    /// The MIME type information.
    #[serde(rename = "mime-type")]
    pub mime_type: Option<MimeType>,

    /// The applications that have accessed the file.
    #[serde(rename = "applications")]
    pub applications: Option<Applications>,
}

/// The MIME type of the file.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct MimeType {
    /// The type of the file (e.g., "text/markdown").
    #[serde(rename = "@type")]
    pub mime_type: String,
}

/// A list of applications that accessed the bookmark.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct Applications {
    // The list of applications.
    // #[serde(rename(deserialize="application", serialize="bookmark:applications"))]
    #[serde(rename = "application", default)]
    pub applications: Vec<Application>,
}

/// An application that accessed the bookmark.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct Application {
    /// The name of the application.
    #[serde(rename = "@name")]
    pub name: String,

    /// The command used to execute the application.
    #[serde(rename = "@exec")]
    pub exec: String,

    /// When the application last modified the bookmark.
    #[serde(rename = "@modified")]
    pub modified: String,

    /// The number of times the application has accessed the bookmark.
    #[serde(rename = "@count")]
    pub count: u32,
}

/// An error that can occur when accessing user places files.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("~/.local/share/user-places.xbel: file does not exist")]
    DoesNotExist,
    #[error("~/.local/share/user-places.xbel: could not deserialize")]
    Deserialization(#[source] DeError),
    #[error("could not serialize new file")]
    Serialization(#[source] Option<DeError>),
    #[error("could not read user places file")]
    Read(#[source] std::io::Error),
    #[error("could not read metadata from path")]
    Metadata(#[source] std::io::Error),
    #[error("could not read generate href from path")]
    Path,
    #[error("could not update user places files")]
    Update,
    
}

/// The path where the user-places.xbel file is expected to be found.
pub fn dir() -> Option<PathBuf> {
    dirs::home_dir().map(|dir| dir.join(USER_PLACES_FILE_SUBPATH))
}

/// Read bookmarks from the user-places.xbel file, creates the file if needed
pub fn read_user_places() -> Result<UserPlaces, Error> {
    let path = dir().ok_or(Error::DoesNotExist)?;
    if !path.exists() {
        let _ = create_empty_user_places_file();
    }
    let file_content = fs::read_to_string(&path).map_err(|err| Error::Read(err))?;
    quick_xml::de::from_str(&file_content).map_err(|err| Error::Deserialization(err))
}

/// Convenience function for parsing the user-places.xbel file in its default location.
pub fn parse_file() -> Result<UserPlaces, Error> {
    let path = dir().ok_or(Error::DoesNotExist)?;
    let file_content = fs::read_to_string(&path).map_err(|err| Error::Read(err))?;
    quick_xml::de::from_str(&file_content).map_err(|err| Error::Deserialization(err))
}

/// Clear the list of user-bookmarked places.
pub fn clear_user_places() -> Result<(), Error> {
    let mut user_places = read_user_places()?;
    user_places.bookmarks.clear();
    return write_user_places(user_places)
}

/// Updates the list of user bookmarked files.
///
/// This function checks if the specified file already exists in the user places list.
/// If it exists, the function updates the file's metadata, including the times when the file was
/// added, modified, and last visited. If the file does not exist in the list, the function adds
/// a new entry for the file.
///
/// If the file already exists in the list, the function also updates the application's usage count,
/// or adds a new application entry if it hasn't been recorded previously.
///
/// # Arguments
///
/// * `element_path` - A `PathBuf` that represents the path to the file being updated or added.
/// * `app_name` - A `String` representing the name of the application associated with the file.
/// * `exec` - A `String` representing the command to execute the application.
/// * `owner` - An optional `String` representing the owner of the metadata. If not provided,
///   defaults to `"http://freedesktop.org"`.
///
/// # Returns
///
/// This function returns `Result<(), Error>`, which is:
/// - `Ok(())` on success.
/// - `Err(Error)` if there is a failure in processing the file (e.g., reading metadata, serialization, or file I/O).
///
/// # Errors
///
/// This function can return errors in the following cases:
///
/// - If the file's metadata cannot be accessed or read.
/// - If the bookmarked file list cannot be parsed or serialized.
/// - If there is an issue writing the updated list back to the file system.
pub fn update_user_place(
    element_path: &PathBuf,
    app_name: String,
    exec: String,
    owner: Option<String>,
) -> Result<(), Error> {
    let owner = match owner {
        Some(owner) => owner,
        None => "http://freedesktop.org".to_string(),
    };
    let mut user_places = read_user_places()?;
    let href = path_to_href(element_path).ok_or(Error::Path)?;
    let metadata = element_path.metadata().map_err(Error::Metadata)?;
    let added = system_time_to_string(metadata.created().map_err(Error::Metadata)?);
    let modified = system_time_to_string(metadata.modified().map_err(Error::Metadata)?);
    let visited = system_time_to_string(metadata.accessed().map_err(Error::Metadata)?);

    // Attempt to find the existing bookmark and update it if found
    let existing_bookmark = user_places.bookmarks.iter_mut().find(|b| b.href == href);

    if let Some(bookmark) = existing_bookmark {
        let modified_clone = modified.clone();

        // Bookmark exists, update the metadata
        bookmark.added = Some(added);
        bookmark.modified = Some(modified_clone);
        bookmark.visited = Some(visited);

        // Find the application entry or insert a new one
        if let Some(info) = bookmark.info.as_mut() {
            let mut info_meta_apps = vec![];
            if Some(info.metadata.applications.clone()).is_some() {
                info_meta_apps = info.metadata.applications.clone().unwrap().applications;
            }
            if let Some(app) = info_meta_apps
                .iter_mut()
                .find(|el| el.name == app_name)
            {
                app.count += 1;
                app.modified = modified.clone();
            } else {
                // Application not found, insert a new one
                info_meta_apps.push(Application {
                    name: app_name,
                    exec,
                    modified: modified.clone(),
                    count: 1,
                });
                info.metadata.applications = Some(Applications { applications: info_meta_apps });
            }
        }
    } else {
        // Bookmark does not exist, create a new one
        let mime = mime_from_path(&element_path).map(|mime| MimeType { mime_type: mime });

        let applications = vec![Application {
            name: app_name,
            exec,
            modified: modified.clone(),
            count: 1,
        }];

        let info = Info {
            metadata: Metadata {
                owner,
                mime_type: mime,
                applications: Some(Applications { applications }),
            },
        };

        let new_bookmark = Bookmark {
            href,
            added: Some(added),
            modified: Some(modified),
            visited: Some(visited),
            info: Some(info),
        };

        user_places.bookmarks.push(new_bookmark);
    }

    return write_user_places(user_places);
}

/// Removes elements from the list of user-bookmarked files.
///
/// # Arguments
///
/// * `element_path` - A `PathBuf` that represents the path to the file being removed.
///
/// # Returns
///
/// This function returns `Result<(), Error>`, which is:
/// - `Ok(())` on success.
/// - `Err(Error)` if there is a failure in processing the file (e.g., reading metadata, serialization, or file I/O).
///
/// # Errors
///
/// This function can return errors in the following cases:
///
/// - If the file's metadata cannot be accessed or read.
/// - If the recently used file list cannot be parsed or serialized.
/// - If there is an issue writing the updated list back to the file system.
pub fn remove_user_place(element_paths: &[&Path]) -> Result<(), Error> {
    let mut user_places = read_user_places()?;
    let mut hrefs = HashSet::with_capacity(element_paths.len());
    for path in element_paths {
        hrefs.insert(path_to_href(path).ok_or(Error::Path)?);
    }
    user_places.bookmarks.retain(|b| !hrefs.contains(&b.href));
    return write_user_places(user_places);
}

fn system_time_to_string(time: SystemTime) -> String {
    let datetime: DateTime<Utc> = time.into();
    datetime.to_rfc3339_opts(SecondsFormat::Micros, true)
}

fn path_to_href(path: &Path) -> Option<String> {
    let path_str = path.to_str()?;
    Url::from_file_path(path_str).ok().map(Into::into)
}

fn mime_from_path(path: &Path) -> Option<String> {
    let path = path.to_string_lossy().to_string();
    let kind = mime_guess::from_path(path);
    let mime = kind.first();
    let mime = match mime {
        Some(mime) => mime,
        None => return None,
    };
    Some(format!("{}/{}", mime.type_(), mime.subtype()))
}

/// Create an empty user places file
fn create_empty_user_places_file() -> Result<(), Error> {
    let empty_user_places = UserPlaces {
        bookmarks: vec![],
        xmlns_mime: String::new(),
        xmlns_bookmark: String::new(),
    };
    return write_user_places(empty_user_places);
}

/// Write out bookmarks to the user places file
fn write_user_places(contents: UserPlaces) -> Result<(), Error> {
    // Prepare the file content
    let serialized = custom_write(contents.clone())?;
    let xml_declaration = r#"<?xml version="1.0" encoding="UTF-8"?>"#;
    let full_content = format!("{}{}", xml_declaration, serialized);

    // Atomically write out the new file
    let path = dir().ok_or(Error::DoesNotExist)?;
    let af = AtomicFile::new(&path, AllowOverwrite);
    af.write(|f| { f.write_all(&full_content.into_bytes()) }).map_err(|_| Error::Update)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs::{self, OpenOptions},
        io::Write,
    };
    use tempfile::tempdir;

    #[test]
    fn test_update_user_place() -> Result<(), Box<dyn std::error::Error>> {

        // Check that we can read from the file.  Creates the file if it doesn't exist
        let content = read_user_places()?;

        // Create a temp file for testing with
        let temp_dir = tempdir()?;
        let temp_file_path = temp_dir.path().join("test_file.txt");
        fs::write(&temp_file_path, b"Test content")?;
        
        // Write the temp file to the user-places.xbel file
        update_user_place(
            &temp_file_path,
            String::from("org.test"),
            String::from("test"),
            None,
        )?;

        // Check for the raw text inside the user-places.xbel file
        let content = fs::read_to_string(&user_places_file_path)?;
        assert!(content.contains(&test_file));

        // Check that the user-places.xbel file can be read correctly
        let user_places = read_user_places()?;

        assert!(user_places.bookmarks.len() > 0);

        // Check that the test bookmark is stored correctly
        let bookmark = user_places
            .bookmarks
            .iter()
            .find(|el| el.href.contains("test_file"));

        assert!(bookmark.is_some());

        // Remove the testing file bookmark
        let length_before_remove = user_places.bookmarks.len();
        remove_user_place(&[&temp_file_path])?;

        // Check that the raw text has been removed
        let content = fs::read_to_string(&user_places_file_path)?;
        assert!(!content.contains(&test_file));

        // Check that the user-places.xbel file can still be read correctly
        let user_places = read_user_places()?;

        assert!(user_places.bookmarks.len() > 0);

        // Check that we haven't lost any other bookmarks
        assert!(user_places.bookmarks.len() == length_before_remove - 1);

        // Check that the testing file bookmark has been removed
        let bookmark = deserialized
            .bookmarks
            .iter()
            .find(|el| el.href.contains("test_file"));

        assert!(bookmark.is_none());

        // Test was successful
        Ok(())
    }

}
