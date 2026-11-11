//! Phase 0 spike: prove WinUI 3 runs from Rust. Throwaway; replaced by the real app shell in Phase 3.

use crate::bindings::{
    Application, ApplicationInitializationCallback, PropertyValue, SwapChainPanel, TabView, TabViewItem,
    TypeKind as BindingsTypeKind, TypeName as BindingsTypeName, Window, XamlControlsResources,
    XamlControlsXamlMetaDataProvider,
};
use crate::composition::{
    Application as ComposedApplication, IApplicationOverrides, IApplicationOverrides_Impl, IXamlMetadataProvider,
    IXamlMetadataProvider_Impl, IXamlType, LaunchActivatedEventArgs, TypeName, XmlnsDefinition,
};
use crate::swap_chain_clear::attach_cleared_swap_chain;
use windows_core::{Array, HSTRING, Interface, Ref, Result, implement};

/// Application subclass. Delegates XAML type lookup for built-in WinUI controls to the framework's own provider.
#[implement(IXamlMetadataProvider, IApplicationOverrides)]
pub struct SpikeApplication {
    controls_metadata: XamlControlsXamlMetaDataProvider,
}

impl IApplicationOverrides_Impl for SpikeApplication_Impl {
    fn OnLaunched(&self, _args: Ref<LaunchActivatedEventArgs>) -> Result<()> {
        merge_control_resources()?;
        build_window()
    }
}

impl IXamlMetadataProvider_Impl for SpikeApplication_Impl {
    fn GetXamlType(&self, type_name: &TypeName) -> Result<IXamlType> {
        let framework_type_name =
            BindingsTypeName { Name: type_name.name.clone(), Kind: BindingsTypeKind(type_name.kind.0) };
        self.controls_metadata.GetXamlType(&framework_type_name)?.cast()
    }

    fn GetXamlTypeByFullName(&self, full_name: &HSTRING) -> Result<IXamlType> {
        self.controls_metadata.GetXamlTypeByFullName(full_name)?.cast()
    }

    fn GetXmlnsDefinitions(&self) -> Result<Array<XmlnsDefinition>> {
        let framework_definitions = self.controls_metadata.GetXmlnsDefinitions()?;
        let definitions: Vec<XmlnsDefinition> = framework_definitions
            .iter()
            .map(|definition| XmlnsDefinition {
                xml_namespace: definition.XmlNamespace.clone(),
                namespace: definition.Namespace.clone(),
            })
            .collect();
        Ok(Array::from_slice(&definitions))
    }
}

pub fn run() -> Result<()> {
    Application::Start(&ApplicationInitializationCallback::new(|_| {
        let composed = ComposedApplication::compose(SpikeApplication {
            controls_metadata: XamlControlsXamlMetaDataProvider::new()?,
        })?;
        let application: Application = composed.cast()?;
        // Keep the application alive for the lifetime of the message loop.
        std::mem::forget(application);
        Ok(())
    }))
}

fn build_window() -> Result<()> {
    let window = Window::new()?;
    window.SetTitle(&HSTRING::from("Winuake spike"))?;

    let tabs = TabView::new()?;
    let tab = TabViewItem::new()?;
    tab.SetHeader(&PropertyValue::CreateString(&HSTRING::from("Terminal 1"))?)?;

    let terminal_surface = SwapChainPanel::new()?;
    terminal_surface.SetWidth(1280.0)?;
    terminal_surface.SetHeight(720.0)?;
    attach_cleared_swap_chain(&terminal_surface, 1280, 720, [0.05, 0.35, 0.40, 1.0])?;
    tab.SetContent(&terminal_surface)?;
    tabs.TabItems()?.Append(&tab)?;

    window.SetContent(&tabs)?;
    window.Activate()
}

fn merge_control_resources() -> Result<()> {
    let resources = Application::Current()?.Resources()?;
    resources.MergedDictionaries()?.Append(&XamlControlsResources::new()?)?;
    Ok(())
}
