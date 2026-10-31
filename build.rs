// Windows: 아이콘과 제작자(OKTO) 정보를 exe에 넣는다. 다른 OS에서는 아무것도 하지 않는다.
fn main() {
    println!("cargo:rerun-if-changed=assets/qrwedge.rc");
    println!("cargo:rerun-if-changed=assets/icon.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        embed_resource::compile("assets/qrwedge.rc", embed_resource::NONE)
            .manifest_optional()
            .unwrap();
    }
}
