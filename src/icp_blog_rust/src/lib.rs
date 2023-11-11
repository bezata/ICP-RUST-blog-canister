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
    fn to_bytes(&self) -> Cow<[u8]> {
        Cow::Owned(Encode!(self).unwrap())
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Self {
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
            msg: format!("Blog post with ID {} not found", id),
        }),
    }
}

// Update function to create a new blog post
#[ic_cdk::update]
fn create_blog_post(payload: BlogPostPayload) -> Option<BlogPost> {
    let id = generate_unique_id()?;

    let blog_post = BlogPost {
        id,
        title: payload.title,
        content: payload.content,
        author: payload.author,
        created_at: time(),
        updated_at: None,
        likes: 0,
        categories: payload.categories,
    };

    do_insert(&blog_post);
    Some(blog_post)
}

fn generate_unique_id() -> Option<u64> {
    let current_value = ID_COUNTER
        .with(|counter| {
            let current_value = *counter.borrow().get();
            counter.borrow_mut().set(current_value + 1)
        })
        .ok()?;

    // Check if the ID counter is out of bounds
    if current_value < u64::MAX {
        Some(current_value)
    } else {
        None
    }
}

// Update function to update an existing blog post
#[ic_cdk::update]
fn update_blog_post(id: u64, payload: BlogPostPayload) -> Result<BlogPost, Error> {
    if _get_blog_post(&id).is_none() {
        return Err(Error::NotFound {
            msg: format!("Blog post with ID {} not found. Cannot update.", id),
        });
    }

    let blog_post = BlogPost {
        id,
        title: payload.title,
        content: payload.content,
        author: payload.author,
        created_at: time(),
        updated_at: Some(time()),
        likes: 0,
        categories: payload.categories,
    };

    do_insert(&blog_post);
    Ok(blog_post)
}

// Update function to delete a blog post by ID
#[ic_cdk::update]
fn delete_blog_post(id: u64) -> Result<BlogPost, Error> {
    match _get_blog_post(&id) {
        Some(blog_post) => {
            if blog_post.likes > 0 {
                return Err(Error::HasLikes {
                    msg: format!("Blog post with ID {} has likes. Cannot delete.", id),
                });
            }

            BLOG_POSTS.with(|service| service.borrow_mut().remove(&id));
            Ok(blog_post)
        }
        None => Err(Error::NotFound {
            msg: format!("Blog post with ID {} not found. Cannot delete.", id),
        }),
    }
}

// Update function to increment the "likes" count of a blog post
#[ic_cdk::update]
fn like_blog_post(id: u64) -> Result<BlogPost, Error> {
    match _get_blog_post(&id) {
        Some(mut blog_post) => {
            if blog_post.likes == u32::MAX {
                return Err(Error::MaxLikes {
                    msg: format!("Blog post with ID {} already at maximum likes.", id),
                });
            }

            blog_post.likes += 1;
            do_insert(&blog_post);
            Ok(blog_post.clone())
        }
        None => Err(Error::NotFound {
            msg: format!("Blog post with ID {} not found. Cannot like.", id),
        }),
    }
}

// Update function to decrement the "likes" count of a blog post
#[ic_cdk::update]
fn dislike_blog_post(id: u64) -> Result<BlogPost, Error> {
    match _get_blog_post(&id) {
        Some(mut blog_post) => {
            if blog_post.likes == 0 {
                return Err(Error::MinLikes {
                    msg: format!("Blog post with ID {} already at minimum likes.", id),
                });
            }

            blog_post.likes -= 1;
            do_insert(&blog_post);
            Ok(blog_post.clone())
        }
        None => Err(Error::NotFound {
            msg: format!("Blog post with ID {} not found. Cannot dislike.", id),
        }),
    }
}

// Define an enum to represent errors, specifically "Not Found" and "Has Likes" errors
#[derive(candid::CandidType, Deserialize, Serialize)]
enum Error {
    NotFound { msg: String },
    HasLikes { msg: String },
    MaxLikes { msg: String },
    MinLikes { msg: String },
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
