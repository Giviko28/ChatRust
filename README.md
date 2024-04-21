# Chat server template

This is a template for writing a chat server and client as your ASP lab project. It contains three projects:
- `server`: This is the server executable, so this is where all your server code will go
- `client`: This is a project for **one** chat client written in Rust. The easiest way would be to use the terminal as a chat client, so this project contains some library code that makes interacting with the terminal easier
    - You are of course free to write more than one client application, you could write one in a different language as well, though the focus of your coding should be on Rust software
- `common`: This is a library project and is included in the `server` and `client` projects. This is where all common code should go, for example the definitions for your datatypes that store messages, users, chat room specifications etc. 