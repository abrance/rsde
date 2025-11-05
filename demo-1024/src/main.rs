use std::time::Instant;

fn compute_factorial(n: u64) -> u64 {
    if n == 0 || n == 1 {
        1
    } else {
        n * compute_factorial(n - 1)
    }
}

#[tokio::main]
async fn main() {
    // // 同时发起多个纯计算任务
    // let task1 = tokio::task::spawn_blocking(|| {
    //     println!("[任务1] 开始计算 factorial(20)");
    //     let result = compute_factorial(20);
    //     println!("[任务1] 完成，结果: {}", result);
    //     result
    // });

    // let result1 = task1.await.unwrap();

    let runtime = tokio::runtime::Runtime::new().unwrap();
    let start = Instant::now();

    let task1: tokio::task::JoinHandle<u64> = tokio::task::spawn_blocking(|| {
        println!("[任务1] 开始计算 factorial(20)");
        let result = compute_factorial(20);
        println!("[任务1] 完成，结果: {}", result);
        result
    });
    let result = runtime.block_on(task1).unwrap();

    let duration = start.elapsed();
    println!("总耗时: {:?}", duration);
    println!("最终结果: {}", result);
}