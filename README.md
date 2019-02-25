# rpc-impl-macro

An alternate macro for paritytech/jsonrpc services, which creates the service
as an `impl` on an item instead of as a separate trait, reducing repetition in
the common case.

This is extracted from some of my own projects which are still in development,
and may break carelessly for the moment. PRs accepted still, though.

## Install

Currently not published on crates.io.

```toml
rpc-impl-macro = { git = "https://github.com/passcod/rpc-impl-macro", tag = "v0.1.0" }
rpc-macro-support = { git = "https://github.com/passcod/rpc-impl-macro", tag = "v0.1.0" }
```

## Use

```rust
use rpc_impl_macro::rpc_impl_struct;
use jsonrpc_core::{IoHandler, Result as RpcResult};

#[derive(Default)]
struct ThingRpc;

rpc_impl_struct! {
    impl ThingRpc {
        pub fn hello(&self, name: String) -> RpcResult<String> {
            Ok(format!("Hello {}!", name))
        }

        #[rpc(name = "hello.custom")]
        pub fn custom_hello(&self, name: String, greeting: String) -> RpcResult<String> {
            Ok(format!("{} {}!", greeting, name))
        }

        #[rpc(notification)]
        pub fn wave(&self, name: String) {
            println!("{} says hello!", name);
        }
    }
}

let mut rpc = IoHandler::new();
rpc.extend_with(ThingRpc::default().to_delegate());
```

- The `impl`'s struct can be named anything.
- Every method in the impl in the macro will become an RPC method. To have
  non-RPC methods on the struct, declare them on a separate `impl` outside the macro.
- RPC methods will take the same name as the Rust method, unless renamed with the
  `#[rpc(name = "new name")]` attribute.
- RPC notifications can be declared with the `#[rpc(notification)]` attribute.

## License

[Artistic License 2.0](./LICENSE), see LICENSE file for details.

Additionally, any suit or legal action relating to this work may only be
brought in New Zealand.
