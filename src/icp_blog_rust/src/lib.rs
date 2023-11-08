#[macro_use]
extern crate serde;
use candid::{Decode, Encode}; // Dependencies for serialization/deserialization
use ic_cdk::api::time; // Time-related functions from the IC SDK
use ic_stable_structures::memory_manager::{MemoryId, MemoryManager, VirtualMemory}; // Custom memory management structures
use ic_stable_structures::{BoundedStorable, Cell, DefaultMemoryImpl, StableBTreeMap, Storable}; // Custom data structures
use std::{borrow::Cow, cell::RefCell};

type Memory = VirtualMemory<DefaultMemoryImpl>;
type IdCell = Cell<u64, Memory>;

// Define a struct representing a blog post
#[derive(candid::CandidType, Clone, Serialize, Deserialize, Default)]
struct BlogPost {
    id: u64,
    title: String,
    content: String,
    author: String,
    created_at: u64,
    updated_at: Option<u64>,
    likes: u32,
    categories: Vec<String>,
}

impl Storable for BlogPost {
    // Implement the `Storable` trait for serialization
    fn to_bytes(&self) -> std::borrow::Cow<[u8]> {
        Cow::Owned(Encode!(self).unwrap())
    }

    fn from_bytes(bytes: std::borrow::Cow<[u8]>) -> Self {
        Decode!(bytes.as_ref(), Self).unwrap()
    }
}

impl BoundedStorable for BlogPost {
    const MAX_SIZE: u32 = 1024; // Maximum size for the serialized data
    const IS_FIXED_SIZE: bool = false; // Data size is not fixed
}

// Thread-local storage for various components
thread_local! {
    static MEMORY_MANAGER: RefCell<MemoryManager<DefaultMemoryImpl>> = RefCell::new(
        MemoryManager::init(DefaultMemoryImpl::default())
    );

    static ID_COUNTER: RefCell<IdCell> = RefCell::new(
        IdCell::init(MEMORY_MANAGER.with(|m| m.borrow().get(MemoryId::new(0))), 0)
            .expect("Cannot create a counter")
    );

    static BLOG_POSTS: RefCell<StableBTreeMap<u64, BlogPost, Memory>> =
        RefCell::new(StableBTreeMap::init(
            MEMORY_MANAGER.with(|m| m.borrow().get(MemoryId::new(1)))
        ));
}

// Define a struct for payload when creating or updating a blog post
#[derive(candid::CandidType, Serialize, Deserialize, Default)]
struct BlogPostPayload {
    title: String,
    content: String,
    author: String,
    categories: Vec<String>,
}

// Query function to get a blog post by ID
#[ic_cdk::query]
fn get_blog_post(id: u64) -> Result<BlogPost, Error> {
    match _get_blog_post(&id) {
        Some(blog_post) => Ok(blog_post),
        None => Err(Error::NotFound {
            msg: format!("Blog post with id={} not found", id),
        }),
    }
}

// Update function to create a new blog post
#[ic_cdk::update]
fn create_blog_post(payload: BlogPostPayload) -> Option<BlogPost> {
    // Generate a new unique ID for the blog post
    let id = ID_COUNTER
        .with(|counter| {
            let current_value = *counter.borrow().get();
            counter.borrow_mut().set(current_value + 1)
        })
        .expect("Cannot increment id counter");

    // Create a new blog post with the provided payload
    let blog_post = BlogPost {
        id,
        title: payload.title,
        content: payload.content,
        author: payload.author,
        created_at: time(), // Set the creation timestamp
        updated_at: None,
        likes: 0,
        categories: payload.categories,
    };

    // Insert the new blog post into the data store
    do_insert(&blog_post);

    Some(blog_post)
}

// Update function to update an existing blog post
#[ic_cdk::update]
fn update_blog_post(id: u64, payload: BlogPostPayload) -> Result<BlogPost, Error> {
    match BLOG_POSTS.with(|service| service.borrow().get(&id)) {
        Some(mut blog_post) => {
            // Update the blog post fields with the new payload
            blog_post.title = payload.title;
            blog_post.content = payload.content;
            blog_post.author = payload.author;
            blog_post.updated_at = Some(time()); // Set the update timestamp
            do_insert(&blog_post); // Update the blog post in the data store
            Ok(blog_post)
        }
        None => Err(Error::NotFound {
            msg: format!(
                "Blog post with id={} not found. Cannot update.",
                id
            ),
        }),
    }
}

// Update function to delete a blog post by ID
#[ic_cdk::update]
fn delete_blog_post(id: u64) -> Result<BlogPost, Error> {
    match BLOG_POSTS.with(|service| service.borrow_mut().remove(&id)) {
        Some(blog_post) => Ok(blog_post),
        None => Err(Error::NotFound {
            msg: format!(
                "Blog post with id={} not found. Cannot delete.",
                id
            ),
        }),
    }
}

// Update function to increment the "likes" count of a blog post
#[ic_cdk::update]
fn like_blog_post(id: u64) -> Result<BlogPost, Error> {
    match BLOG_POSTS.with(|service| service.borrow().get(&id)) {
        Some(mut blog_post) => {
            blog_post.likes += 1;
            do_insert(&blog_post); // Update the blog post in the data store
            Ok(blog_post.clone())
        }
        None => Err(Error::NotFound {
            msg: format!(
                "Blog post with id={} not found. Cannot like.",
                id
            ),
        }),
    }
}

// Update function to decrement the "likes" count of a blog post
#[ic_cdk::update]
fn dislike_blog_post(id: u64) -> Result<BlogPost, Error> {
    match BLOG_POSTS.with(|service| service.borrow().get(&id)) {
        Some(mut blog_post) => {
            blog_post.likes -= 1;
            do_insert(&blog_post); // Update the blog post in the data store
            Ok(blog_post.clone())
        }
        None => Err(Error::NotFound {
            msg: format!(
                "Blog post with id={} not found. Cannot dislike.",
                id
            ),
        }),
    }
}

// Define an enum to represent errors, specifically "Not Found" errors
#[derive(candid::CandidType, Deserialize, Serialize)]
enum Error {
    NotFound { msg: String },
}

// Helper function to insert a blog post into the data store
fn do_insert(blog_post: &BlogPost) {
    BLOG_POSTS.with(|service| service.borrow_mut().insert(blog_post.id, blog_post.clone()));
}

// Helper function to retrieve a blog post by ID
fn _get_blog_post(id: &u64) -> Option<BlogPost> {
    BLOG_POSTS.with(|service| service.borrow().get(id))
}

// Export Candid interface for the Dapp
ic_cdk::export_candid!();
