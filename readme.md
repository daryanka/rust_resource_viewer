# Resource Viewer
## Simple project to help with learning rust

<img src="assets/demo.gif" alt="Demo recording"/>

### Reason for making this:
Rather than continuing to read about or watch videos about rust, I mostly built this project to actually build something with rust.

I definatly made a lot of newbie mistakes.

### Issues I ran into:
I had problems with lifetimes where some of the variables in the Charts could outlive the data if the data were created inside the functions. For instance, in the function `cpu_block`, if I created the `vec` that would be passed into the dataset inside the function, I would receive an error that the `vec` is dropped when the function ends, but the chart would live on because it is returned and ownership is transferred. The solution I came up with is extremely hacky as I ended up just putting the data inside the state.

I also definatly didn't follow the right structure for using the `tui` create, as my app state was not being stored in the right place. I think there is a way to directly embedded it in the terminal or frame. This might have solved some of the lifetimes issues but im not sure.

I wasn't sure how best to share data between threads, as I ended up having the system data being updated in a different thread than the main render loop. I ended up using a `Arc` with a `RwLock`. Im not sure if this is the best approach or if there is something else that would have less overhead.

### Things I enjoyed:

It was a lot simpler than I expected it to be, most likly due to how well made the `tui` crate it.

When writing in rust, since the type system is so extensive, if my code compiles, then it usually just worked as intented.

The error messages where very clear and showed exactly why it was erroring.

### Things I didn't enjoy:

While the error messages where very descriptive, for the more complex/uncommon errors, such as the lifetimes issues I ran into. It wasn't clear what I could do to fix it.

The number of online resources available isn't on the same scale as more popular languages such as JS/TS. This meant that I spent more time looking for an answer to something than I would usually spend on another language.

The traits systems is sometimes confusing, when trying to access a trait on the `Networks` struct, I could see in the source code it was there, but I couldn't use it which was very confusing. It turned out you have to add `use sysinfo::NetworkExt;` to use those traits. This pattern is probably something you get used to but can be very confusin to begin with.

### Usage:

To run this locally you would need to clone to repo, then run `cargo run`, this does require rust and cargo to be installed on your system.