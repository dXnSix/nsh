//! What a host gives one engine: typed values its UI sets' Rust code reads
//! while that engine runs it ([`engine_context`]).
//!
//! Several engines run in one process (one per surface), and a module
//! function is a free function: what it reaches of its host would be
//! process-wide without this. The engine makes its context its thread's
//! ([`EngineContext::enter`]) for its whole run, so module functions read
//! it in the JS call, as ops and as `async` tasks (polled on that thread),
//! and so do components. Work sent to other threads (`background`) does
//! not see it: read it first and move the value there.
//!
//! The engine adds what the app's Rust code reads of the app itself, its id
//! and its own directories ([`app_id`], [`data_dir`], [`cache_dir`]), from
//! its config.

use std::{
    any::{Any, TypeId},
    cell::RefCell,
    fmt,
    marker::PhantomData,
    path::PathBuf,
    rc::Rc,
    sync::Arc,
};

use rustc_hash::FxHashMap;

type Values = FxHashMap<TypeId, Arc<dyn Any + Send + Sync>>;

/// Typed values a host gives one engine (`EngineConfig::context`), at most
/// one per type, which its Rust code reads with [`engine_context`].
#[derive(Clone, Default)]
#[non_exhaustive]
pub struct EngineContext {
    values: Arc<Values>,
}

impl EngineContext {
    /// This context with `value` as its `T`, replacing one it had.
    pub fn with<T: Send + Sync + 'static>(mut self, value: T) -> Self {
        Arc::make_mut(&mut self.values).insert(TypeId::of::<T>(), Arc::new(value));
        self
    }

    /// Its `T`, if it has one.
    pub fn get<T: Send + Sync + 'static>(&self) -> Option<Arc<T>> {
        self.values
            .get(&TypeId::of::<T>())
            .cloned()
            .and_then(|value| value.downcast::<T>().ok())
    }

    /// This context with the id of the app the engine runs, which its Rust
    /// code reads ([`app_id`]). The engine sets it from its app, replacing
    /// one the host gave.
    pub fn with_app_id(self, id: Option<String>) -> Self {
        self.with(AppId(id))
    }

    /// This context with the app's own data directory, which its Rust code
    /// reads ([`data_dir`]). The engine sets it from its config, replacing
    /// one the host gave.
    pub fn with_data_dir(self, dir: Option<PathBuf>) -> Self {
        self.with(DataDir(dir))
    }

    /// This context with the app's own cache directory, which its Rust code
    /// reads ([`cache_dir`]). The engine sets it from its config, replacing
    /// one the host gave.
    pub fn with_cache_dir(self, dir: Option<PathBuf>) -> Self {
        self.with(CacheDir(dir))
    }

    /// Makes it the current thread's until the guard drops: what
    /// [`engine_context`] reads there (the engine does this on its thread).
    pub fn enter(&self) -> EngineContextGuard {
        let previous = CURRENT.with(|current| current.replace(Some(self.clone())));
        EngineContextGuard {
            previous,
            _thread: PhantomData,
        }
    }
}

impl fmt::Debug for EngineContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EngineContext")
            .field("values", &self.values.len())
            .finish()
    }
}

thread_local! {
    /// The context of the engine on this thread.
    static CURRENT: RefCell<Option<EngineContext>> = const { RefCell::new(None) };
}

/// Puts the thread's previous context back ([`EngineContext::enter`]).
///
/// It stays on the thread that entered. Its drop writes that thread's
/// context, so the guard is not [`Send`].
#[must_use = "the context is the thread's while the guard lives"]
pub struct EngineContextGuard {
    previous: Option<EngineContext>,
    /// `Rc` is not `Send`: the guard cannot move to a thread whose
    /// thread-local it must not write.
    _thread: PhantomData<Rc<()>>,
}

// `Send` would make `EngineContextGuard::check` ambiguous (both impls
// apply). A guard that moves to another thread would restore this
// thread's previous context there.
const _: fn() = || {
    trait AmbiguousIfSend<A> {
        fn check() {}
    }
    impl<T: ?Sized> AmbiguousIfSend<()> for T {}
    impl<T: ?Sized + Send> AmbiguousIfSend<u8> for T {}
    let _ = EngineContextGuard::check;
};

impl Drop for EngineContextGuard {
    fn drop(&mut self) {
        let previous = self.previous.take();
        // Host values may read the context during cleanup; release the
        // mutable borrow before dropping the context that owns them.
        let outgoing = CURRENT.with(|current| current.replace(previous));
        drop(outgoing);
    }
}

/// The `T` the host gave the engine running this code
/// ([`EngineContext::with`]); `None` without one, or off the engine's
/// thread.
pub fn engine_context<T: Send + Sync + 'static>() -> Option<Arc<T>> {
    CURRENT.with(|current| current.borrow().as_ref()?.get::<T>())
}

/// [`EngineContext::with_app_id`]'s value: a type of its own, which only
/// this module names.
struct AppId(Option<String>);

/// [`EngineContext::with_data_dir`]'s value.
struct DataDir(Option<PathBuf>);

/// [`EngineContext::with_cache_dir`]'s value.
struct CacheDir(Option<PathBuf>);

/// The app's id, `nsh.json`'s `"id"` (`com.example.notes`), which travels
/// with the app (its package, the dev server's bundle, [`Root::app_id`] in
/// a Rust app's generated code); `None` for an app without one. Code that
/// several apps share (a UI set's crate) tells them apart by it. It does not
/// change while the app runs. Like [`engine_context`], it is read on the
/// engine's thread (a component, a module function, a task) and is `None`
/// off it: read it before sending work to `background`.
///
/// [`Root::app_id`]: super::Root::app_id
pub fn app_id() -> Option<String> {
    engine_context::<AppId>()?.0.clone()
}

/// The app's own data directory, for the files it keeps (TS:
/// `process.dataDir`, the capabilities' `${dataDir}`): the directory of the
/// app's id ([`app_id`]; `default` without one) in the place the platform
/// keeps apps' data (Application Support on macOS, `nsh` in it on iOS,
/// `%APPDATA%` on Windows, `$XDG_DATA_HOME` on Linux, `nsh` in the files
/// directory on Android) or in the one the embedding app chose.
/// NSH keeps the app's [`Storage`](crate::storage::Storage) there too, in
/// `storage.json`. `None` where the host keeps no files of the app's: the
/// web, where Rust code keeps its data in `Storage`. The directory may not
/// exist yet: create it before the first write (`std::fs::create_dir_all`
/// in `blocking(…)`). It does not change while the app runs; read on the
/// engine's thread, as [`app_id`].
pub fn data_dir() -> Option<PathBuf> {
    engine_context::<DataDir>()?.0.clone()
}

/// The app's own cache directory, for files it can make again (downloads,
/// thumbnails): the directory of the app's id in the platform's cache
/// directory (Caches on macOS, `nsh` in it on iOS, `%LOCALAPPDATA%` on
/// Windows, `$XDG_CACHE_HOME` on Linux, `nsh` in the cache directory on
/// Android), which the system may clear to free space, or in the one the
/// embedding app chose.
/// NSH's own caches are directories in it (`images`, the images the app
/// downloads). `None` where the host caches nothing on disk: the web, and
/// a host that turned the app's caches off. The directory may not exist
/// yet. It does not change while the app runs; read on the engine's thread,
/// as [`app_id`].
pub fn cache_dir() -> Option<PathBuf> {
    engine_context::<CacheDir>()?.0.clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_thread_reads_the_context_it_entered_until_the_guard_drops() {
        let outer = EngineContext::default().with(1_u32).with("outer");
        let inner = EngineContext::default().with(2_u32);
        assert_eq!(engine_context::<u32>(), None);
        {
            let _outer = outer.enter();
            assert_eq!(engine_context::<u32>().as_deref(), Some(&1));
            {
                let _inner = inner.enter();
                assert_eq!(engine_context::<u32>().as_deref(), Some(&2));
                assert_eq!(engine_context::<&str>(), None);
            }
            assert_eq!(engine_context::<&str>().as_deref(), Some(&"outer"));
        }
        assert_eq!(engine_context::<u32>(), None);
    }

    #[test]
    fn releasing_context_values_allows_them_to_read_the_restored_context() {
        use std::sync::atomic::{AtomicBool, Ordering};

        struct HostState(Arc<AtomicBool>);
        impl Drop for HostState {
            fn drop(&mut self) {
                assert_eq!(engine_context::<u32>().as_deref(), Some(&7));
                self.0.store(true, Ordering::SeqCst);
            }
        }

        let _outer = EngineContext::default().with(7_u32).enter();
        let released = Arc::new(AtomicBool::new(false));
        // Only the thread-local owns this context when its guard exits,
        // as after LocalEngine releases its driver during shutdown.
        let inner = EngineContext::default()
            .with(HostState(released.clone()))
            .enter();
        drop(inner);
        assert!(released.load(Ordering::SeqCst));
        assert_eq!(engine_context::<u32>().as_deref(), Some(&7));
    }

    #[test]
    fn the_apps_id_and_directories_are_its_engines_and_the_last_set_wins() {
        assert_eq!((app_id(), data_dir(), cache_dir()), (None, None, None));
        let host = EngineContext::default()
            .with_app_id(Some("host".into()))
            .with_data_dir(Some("/host".into()));
        let engine = host
            .with_app_id(Some("com.example.notes".into()))
            .with_data_dir(None)
            .with_cache_dir(Some("/caches/com.example.notes".into()));
        {
            let _entered = engine.enter();
            assert_eq!(app_id().as_deref(), Some("com.example.notes"));
            assert_eq!(data_dir(), None);
            assert_eq!(
                cache_dir(),
                Some(PathBuf::from("/caches/com.example.notes"))
            );
            // Off the engine's thread, none.
            let elsewhere = std::thread::spawn(|| (app_id(), data_dir(), cache_dir()));
            assert_eq!(elsewhere.join().unwrap(), (None, None, None));
        }
        assert_eq!((app_id(), data_dir(), cache_dir()), (None, None, None));
    }
}
