use std::sync::{Arc, LazyLock, Mutex};

pub static CLEANUP_HANDLER: LazyLock<CleanupHandler> = LazyLock::new(CleanupHandler::new);

pub struct RegisteredCleanupFunction {
    pub handler: Box<dyn FnOnce() + Send + Sync>,
    pub name: String,
}

#[derive(Clone)]
pub struct CleanupHandler {
    registered: Arc<Mutex<Vec<RegisteredCleanupFunction>>>,
    terminated: Arc<Mutex<bool>>,
}

impl CleanupHandler {
    fn new() -> Self {
        let handler = CleanupHandler {
            registered: Arc::new(Mutex::new(Vec::new())),
            terminated: Arc::new(Mutex::new(false)),
        };

        let mut handler_ = handler.clone();

        ctrlc::set_handler(move || {
            if !handler_.registered.lock().unwrap().is_empty() {
                println!();
                warn!("Termination signal received, executing registered functions.");
                handler_.terminate();
            }
            std::process::exit(1);
        })
        .expect("Should be able to set termination handler");

        handler
    }

    pub fn register(
        &self,
        name: impl Into<String> + Clone,
        handler: impl FnOnce() + Send + Sync + 'static,
    ) {
        trace!("Registering cleanup function for {}", name.clone().into());
        self.registered
            .lock()
            .unwrap()
            .push(RegisteredCleanupFunction {
                handler: Box::new(handler),
                name: name.into(),
            });
    }

    fn terminate(&mut self) {
        let mut terminated = self.terminated.lock().unwrap();
        if *terminated {
            return;
        }
        *terminated = true;
        for f in (*self.registered.lock().unwrap()).drain(..) {
            info!("Executing cleanup function: {}", f.name);
            (f.handler)();
        }
    }
}

impl Drop for CleanupHandler {
    fn drop(&mut self) {
        if !self.registered.lock().unwrap().is_empty() {
            println!();
            warn!("Cleanup handler dropped, executing registered functions.");
            self.terminate();
        }
    }
}

#[macro_export]
macro_rules! register_cleanup {
    ($name:expr, $handler:expr) => {
        CLEANUP_HANDLER.register($name, $handler);
    };
}

#[macro_export]
macro_rules! handle_cleanup {
    () => {
        drop(CLEANUP_HANDLER.clone());
    };
}
