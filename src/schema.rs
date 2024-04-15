use async_graphql::{EmptySubscription, Object, Result, Schema, SimpleObject, Upload, Context};
use lazy_static::lazy_static;
use tokio::fs;
use std::path::PathBuf;

#[derive(SimpleObject)]
struct User {
    id: i32,
    name: String,
    image_url: String,
}

pub(crate) struct Query;

#[Object]
impl Query {
    async fn users(&self) -> Result<Vec<User>> {
        let users: Vec<User> = vec![
            User {
                id: 1,
                name: "Alex".into(),
                image_url: "https://avatars.githubusercontent.com/u/4726920?v=4".into(),
            },
            User {
                id: 2,
                name: "Jesse".into(),
                image_url: "https://avatars.githubusercontent.com/u/4726920?v=4".into(),
            },
            User {
                id: 3,
                name: "Chamindu".into(),
                image_url: "https://avatars.githubusercontent.com/u/4726920?v=4".into(),
            },
            User {
                id: 4,
                name: "Yasitha".into(),
                image_url: "https://avatars.githubusercontent.com/u/4726920?v=4".into(),
            },
        ];
        tracing::info!("Finished query");
        Ok(users)
    }
}

pub(crate) struct Mutation;

#[Object]
impl Mutation {
    async fn create_user(&self,ctx: &Context<'_>, name: String, image: Upload,image2: Upload) -> Result<User> {
        tracing::info!("User creation with image started");

     //   println!("Image 1 name");

        println!("Image 1 name {}",image.value(ctx).unwrap().filename);

        // // Path where the image will be stored
        // let root_path = PathBuf::from("/imgs");  // Adjust this path to your appropriate root directory
        // let file_path = root_path.join(image.value(ctx).unwrap().filename);

        // // Save the file to the filesystem asynchronously
        // fs::write(&file_path, image.value(ctx).unwrap().content).await?;

        // // Generating a URL or relative path to the image (if necessary)
        // let image_url = format!("/{}", image.value(ctx).unwrap().filename);  // Adjust according to how you access files

        // Placeholder for database insert logic
        // Return the new user object with an image URL
        Ok(User {
            id: 1,  // This should be replaced by the ID assigned by your database or user management system
            name,
            image_url: "https://avatars.githubusercontent.com/u/4726920?v=4".into(),
        })
    }
}

lazy_static! {
    pub(crate) static ref SCHEMA: Schema<Query, Mutation, EmptySubscription> =
        Schema::build(Query, Mutation, EmptySubscription).finish();
}

#[cfg(test)]
mod test_users {
    use serde_json::json;

    use super::SCHEMA;

    #[tokio::test]
    async fn test_create_and_get_user() {
        let res = SCHEMA
            .execute("mutation {createUser(name: \"Alex\") {name}}")
            .await;
        let data = res.data.into_json().expect("Result was not JSON");
        let name = data["createUser"]["name"]
            .as_str()
            .expect("Result was not string");
        assert_eq!(name, "Alex");
        let res = SCHEMA
            .execute("{users{id, name}}")
            .await
            .data
            .into_json()
            .unwrap();
        assert_eq!(
            res,
            json!({"users": [{"name": "Alex", "id": 1}, {"name": "Jesse", "id": 2}]})
        )
    }
}
