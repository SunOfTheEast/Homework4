use std::future::Future;
use std::sync::Arc;
use std::cell::RefCell;
use std::task::Waker;
use std::task::Poll;
use std::task::Context;
use std::sync::Condvar;
use std::task::Wake;
use futures::future::BoxFuture;
use std::sync::Mutex;
use std::time::Duration;
use std::collections::VecDeque;
use async_channel::bounded;
use futures::FutureExt;

struct Demo;

impl Future for Demo {
    type Output = ();

    fn poll(self: std::pin::Pin<&mut Self>, _cx: &mut std::task::Context<'_>) -> std::task::Poll<Self::Output> {
        println!("hello world");
        std::task::Poll::Ready(())
    }
}
scoped_tls::scoped_thread_local!(static SIGNAL: Arc<Signal>);
scoped_tls::scoped_thread_local!(static RUNNABLE: Mutex<VecDeque<Arc<Task>>>);
pub(crate) fn block_on<F: Future>(future: F) -> F::Output {
    let mut main_fut = std::pin::pin!(future);
    let signal =  Arc::new(Signal::new());
    let waker = Waker::from(signal.clone());
    let runnable = Mutex::new(VecDeque::with_capacity(1024));
    let mut cx = Context::from_waker(&waker);
    SIGNAL.set(&signal, || {
        RUNNABLE.set(&runnable, ||
            {
                loop {
                    if let Poll::Ready(output) = main_fut.as_mut().poll(&mut cx) {
                        return output;
                    }
                    while let Some(task) = runnable.lock().unwrap().pop_front() {
                        let waker = Waker::from(task.clone());
                        let mut cx = Context::from_waker(&waker);
                        let _ = task.future.borrow_mut().as_mut().poll(&mut cx);
                    }
                    signal.wait();
                }
            })
    })
}
/////////////////////////////////////////////////////////
struct Signal {
    state: Mutex<State>,
    cond: Condvar,
}

enum State {
    Empty,
    Waiting,
    Notified,
}

impl Signal {
    fn new() -> Signal {
        Signal { state: std::sync::Mutex::new(State::Notified), cond: Condvar::new() }
    }

    fn wait(&self) {
        let mut state = self.state.lock().unwrap();
        match *state {
            State::Notified => *state = State::Empty,
            State::Waiting => {
                panic!("multiple wait");
            }
            State::Empty => {
                *state = State::Waiting;
                while let State::Waiting = *state {
                    state = self.cond.wait(state).unwrap();
                }
            }
        }
    }

    fn notify(&self) {
        let mut state = self.state.lock().unwrap();
        match *state {
            State::Notified => {}
            State::Empty => *state = State::Notified,
            State::Waiting => {
                *state = State::Empty;
                self.cond.notify_one();
            }
        }
    }
}
impl Wake for Signal {
    fn wake(self: Arc<Self>) {
        self.notify();
    }
}
struct Task {
    future: RefCell<BoxFuture<'static, ()>>,
    signal: Arc<Signal>,
}
unsafe impl Send for Task {

}
unsafe impl Sync for Task {

}

impl Wake for Task {
    fn wake(self: Arc<Self>) {
        RUNNABLE.with(|runnable: &Mutex<VecDeque<Arc<Task>>>| runnable.lock().unwrap().push_back(self.clone()));
        self.signal.notify();
    }
}
pub fn spawn<F: Future<Output = ()> + std::marker::Send + 'static>(fut: F) {
    let t = Arc::new(Task{future: RefCell::new(fut.boxed()), signal: Arc::new(Signal::new())});
    RUNNABLE.with(|runnable: &Mutex<VecDeque<Arc<Task>>>| runnable.lock().unwrap().push_back(t));
}