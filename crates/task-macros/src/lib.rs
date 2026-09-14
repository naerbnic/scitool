use scidev_task_impl::macros;

#[proc_macro_attribute]
pub fn task_struct(
    args: proc_macro::TokenStream,
    input: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    macros::result_to_tokens(macros::task_struct(args.into(), input.into())).into()
}
