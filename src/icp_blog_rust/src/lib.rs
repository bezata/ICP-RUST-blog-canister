
#[macro_use]
extern crate serde;
use candid::{Decode, Encode};
use ic_cdk::api::time;
use ic_stable_structures::memory_manager::{MemoryId, MemoryManager, VirtualMemory};
use ic_stable_structures::{BoundedStorable, Cell, DefaultMemoryImpl, StableBTreeMap, Storable};
use std::{borrow::Cow, cell::RefCell};

type Memory = VirtualMemory<DefaultMemoryImpl>;
type IdCell = Cell<u64, Memory>;

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
    fn to_bytes(&self) -> std::borrow::Cow<[u8]> {
        Cow::Owned(Encode!(self).unwrap())
    }

    fn from_bytes(bytes: std::borrow::Cow<[u8]>) -> Self {
        Decode!(bytes.as_ref(), Self).unwrap()
    }
}

impl BoundedStorable for BlogPost {
    const MAX_SIZE: u32 = 1024;
    const IS_FIXED_SIZE: bool = false;
}

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

#[derive(candid::CandidType, Serialize, Deserialize, Default)]
struct BlogPostPayload {
    title: String,
    content: String,
    author: String,
    categories: Vec<String>,
}

#[ic_cdk::query]
fn get_blog_post(id: u64) -> Result<BlogPost, Error> {
    match _get_blog_post(&id) {
        Some(blog_post) => Ok(blog_post),
        None => Err(Error::NotFound {
            msg: format!("Blog post with id={} not found", id),
        }),
    }
}

#[ic_cdk::update]
fn create_blog_post(payload: BlogPostPayload) -> Option<BlogPost> {
    let id = ID_COUNTER
        .with(|counter| {
            let current_value = *counter.borrow().get();
            counter.borrow_mut().set(current_value + 1)
        })
        .expect("Cannot increment id counter");

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

#[ic_cdk::update]
fn update_blog_post(id: u64, payload: BlogPostPayload) -> Result<BlogPost, Error> {
    match BLOG_POSTS.with(|service| service.borrow().get(&id)) {
        Some(mut blog_post) => {
            blog_post.title = payload.title;
            blog_post.content = payload.content;
            blog_post.author = payload.author;
            blog_post.updated_at = Some(time());
            do_insert(&blog_post);
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

#[ic_cdk::update]
fn like_blog_post(id: u64) -> Result<BlogPost, Error> {
    match BLOG_POSTS.with(|service| service.borrow().get(&id)) {
        Some(mut blog_post) => {
            blog_post.likes += 1;
            do_insert(&blog_post);
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

#[ic_cdk::update]
fn dislike_blog_post(id: u64) -> Result<BlogPost, Error> {
    match BLOG_POSTS.with(|service| service.borrow().get(&id)) {
        Some(mut blog_post) => {
            blog_post.likes -= 1;
            do_insert(&blog_post);
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


#[derive(candid::CandidType, Deserialize, Serialize)]
enum Error {
    NotFound { msg: String },
}

fn do_insert(blog_post: &BlogPost) {
    BLOG_POSTS.with(|service| service.borrow_mut().insert(blog_post.id, blog_post.clone()));
}

fn _get_blog_post(id: &u64) -> Option<BlogPost> {
    BLOG_POSTS.with(|service| service.borrow().get(id))
}

ic_cdk::export_candid!();