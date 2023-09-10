mod runtime;

fn main() {
    runtime::block_on(demo());
    println!("Run the demo test that waits for network for 10s. You can use htop to check the CPU usage.");
}

async fn demo() {
    let (tx, rx) = async_channel::bounded::<()>(1);
    runtime::spawn(demo2(tx));
    println!("hello world!");
    let _ = rx.recv().await;
}

async fn demo2(tx: async_channel::Sender<()>) {
    println!("hello world2!");
    let _ = tx.send(()).await;
}