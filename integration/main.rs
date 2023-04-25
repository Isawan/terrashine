mod mock;

use core::future::Future;
use libtest_mimic::{Arguments, Failed, Trial};
use tokio::runtime::Runtime;

fn async_test<F: Future<Output = ()>>(future: F) -> impl FnOnce() -> Result<(), Failed>
where
{
    let rt = Runtime::new().unwrap();
    move || {
        rt.block_on(future);
        Ok(())
    }
}

#[tokio::main]
async fn main() {
    let args = Arguments::from_args();
    let tests = vec![Trial::test(
        "test_binary_able_to_start_up",
        async_test(mock::test_binary_able_to_start_up()),
    )];

    libtest_mimic::run(&args, tests).exit();
}
