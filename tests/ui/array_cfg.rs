use hibana_runtime::scope;

#[scope]
async fn program() -> Result<(), ()> {
    for _ in [()] {
        #[cfg(any())]
        async {
            undefined_after_cfg();
        };
    }
    let value = async {
        for _ in [()] {
            #[cfg_attr(all(), cfg_attr(all(), cfg(any())))]
            async {
                undefined_after_cfg_attr();
                Ok(17u8)
            };
        }
    }
    .await?;
    let _: () = value;
    #[cfg(any())]
    for _ in undefined_array_input() {
        async {
            undefined_after_outer_cfg();
        };
    }
    Ok(())
}

fn main() {
    drop(program());
}
