// Some thoughts on the chat server:
// - This is a server, so an executable that runs perpetually! So there will be a loop, maybe? What will that loop do?
// - At some point, you want to configure your server: Where should it run? Maybe limit the number of concurrent users? What else would you like to configure? How would you do the configuring?
// - Users should be able to message each other. What types of messaging do you want to support? Only one-on-one or also rooms/groups etc? How will the messages look like? Should users be able to send each other files?
// - What job does the server have when it comes to messages? Does it only facilitate peer-to-peer communication between clients, or do all messages go through the server?
//   - What would be the benefits and drawbacks of each approach?
// - Do you want/need some form of user management? If so, how would that look like?

use std::time::Duration;

fn main() {
    loop {
        println!(
            "This is a chat server written in Rust. Currently it only loops but does nothing..."
        );
        std::thread::sleep(Duration::from_secs(1));
    }
}
