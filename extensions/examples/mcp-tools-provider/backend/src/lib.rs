//! Disposable real Provider fixture; uses the public sandbox ABI without trust bypasses.
mod bindings {
    wit_bindgen::generate!({path: ["../../../../specs/extensions/wit/mcp-tools-v1","../../../../specs/extensions/wit/cli-commands-v1/world.wit"],
        inline: "package fixtures:entry; world fixture { include alcomd:extension/portable-ui-v1@1.0.0; import alcomd:extension/host-operations@1.0.0; export alcomd:mcp-tools/provider@1.0.0; export alcomd:cli-commands/provider@1.0.0; }",
        world: "fixture", generate_all});
    #[cfg(target_arch="wasm32")]
    use super::Fixture;
    #[cfg(target_arch="wasm32")]
    export!(Fixture with_types_in self);
}
use bindings::{alcomd::extension::{types, host_operations as core},exports::alcomd::{extension::{guest_lifecycle,guest_ui},mcp_tools::provider}};
use serde_json::{Value,json};
struct Fixture;
impl guest_lifecycle::Guest for Fixture {
    fn activate(_:types::ActivationContext)->Result<(),types::ExtensionError>{Err(types::ExtensionError{code:types::ErrorCode::InvalidArgument,diagnostic_id:None})}
    fn deactivate(_:types::StopReason)->Result<(),types::ExtensionError>{Ok(())}
}
fn document()->guest_ui::UiDocument {guest_ui::UiDocument{title:"MCP tool fixture".into(),nodes:vec![guest_ui::UiNode{node_id:"page".into(),parent_id:None,order:0,payload:guest_ui::NodePayload::Page(guest_ui::PageNode{title:"MCP tool fixture".into()})}]}}
impl guest_ui::Guest for Fixture {
    fn open(_:String,_:String)->guest_ui::UiDocument{document()}
    fn refresh(_:String)->guest_ui::UiDocument{document()}
    fn dispatch(_:String,_:guest_ui::UiAction)->guest_ui::UiDocument{document()}
    fn close(_:String){}
}
impl bindings::exports::alcomd::cli_commands::provider::Guest for Fixture {
    fn invoke(name: String, arguments: Vec<u8>) -> Result<Vec<u8>, String> {
        if name == "echo" {
            let input: Value = serde_json::from_slice(&arguments).map_err(|_| "invalid command input")?;
            return serde_json::to_vec(&json!({"cli":input["text"]})).map_err(|_| "invalid command output".into());
        }
        <Self as provider::Guest>::invoke(name, arguments)
    }
}
impl provider::Guest for Fixture {
    fn invoke(name:String,arguments:Vec<u8>)->Result<Vec<u8>,String>{
        let value:Value=serde_json::from_slice(&arguments).map_err(|_|"invalid input")?;
        let text=|key:&str|value[key].as_str().map(str::to_owned).ok_or_else(||"missing field".to_owned());
        let failure=|_:core::Error|"Core rejected request".to_owned();
        match name.as_str(){
            "echo"=>serde_json::to_vec(&json!({"text":value["text"]})).map_err(|_|"output".into()),
            "projects"=>core::list_projects(None,Some(100)).map_err(failure),
            "project"|"unauthorized_read"=>core::get_project(&text("project_id")?).map_err(failure),
            "plan_remove"|"plan_twice"=>{let args=(text("project_id")?,value["expected_revision"].as_u64().ok_or("revision")?,text("package_id")?);let first=core::plan_uninstall(&args.0,args.1,&args.2).map_err(failure)?;if name=="plan_twice"{assert!(core::plan_uninstall(&args.0,args.1,&args.2).is_err());}Ok(first)},
            "apply"|"bad_after_apply"|"trap_after_apply"|"loop_after_apply"|"apply_twice"=>{let first=core::apply_plan().map_err(failure)?;match name.as_str(){"bad_after_apply"=>Ok(b"not JSON".to_vec()),"trap_after_apply"=>panic!("fixture trap"),"loop_after_apply"=>{let mut n=0u64;loop{n=n.wrapping_add(1);std::hint::black_box(n);}},"apply_twice"=>{assert!(core::apply_plan().is_err());Ok(first)},_=>Ok(first)}},
            "operation"=>core::get_operation(&text("operation_id")?).map_err(failure),
            "cancel"=>core::cancel_operation().map_err(failure),
            _=>Err("unknown tool".into())
        }
    }
}
